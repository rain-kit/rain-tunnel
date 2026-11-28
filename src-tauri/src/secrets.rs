use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::model::Tunnel;
use crate::util::write_file_atomic;

pub const PASSWORD: &str = "password";
pub const KEY_PASSPHRASE: &str = "keyPassphrase";
pub const PROXY_PASS: &str = "proxyPass";
const KINDS: [&str; 3] = [PASSWORD, KEY_PASSPHRASE, PROXY_PASS];

/// 密码存储后端：正式运行用加密文件，测试用内存实现。
pub trait SecretBackend: Send + Sync {
    /// 不存在时返回空字符串。
    fn get(&self, tunnel_id: &str, kind: &str) -> Result<String, String>;
    fn set(&self, tunnel_id: &str, kind: &str, value: &str) -> Result<(), String>;
    fn delete(&self, tunnel_id: &str, kind: &str) -> Result<(), String>;
}

pub struct Secrets {
    backend: Box<dyn SecretBackend>,
}

impl Secrets {
    pub fn new(backend: Box<dyn SecretBackend>) -> Self {
        Secrets { backend }
    }

    pub fn get(&self, tunnel_id: &str, kind: &str) -> Result<String, String> {
        self.backend.get(tunnel_id, kind)
    }

    pub fn has(&self, tunnel_id: &str, kind: &str) -> bool {
        self.get(tunnel_id, kind).is_ok_and(|v| !v.is_empty())
    }

    /// value 为空表示删除。
    pub fn set(&self, tunnel_id: &str, kind: &str, value: &str) -> Result<(), String> {
        if value.is_empty() {
            self.backend.delete(tunnel_id, kind)
        } else {
            self.backend.set(tunnel_id, kind, value)
        }
    }

    pub fn delete_all(&self, tunnel_id: &str) -> Result<(), String> {
        let errs: Vec<String> = KINDS
            .iter()
            .filter_map(|k| self.backend.delete(tunnel_id, k).err())
            .collect();
        if errs.is_empty() { Ok(()) } else { Err(errs.join("; ")) }
    }

    /// 把已保存的密码填回隧道定义，用于启动隧道。
    pub fn fill(&self, t: &mut Tunnel) -> Result<(), String> {
        t.password = self.get(&t.id, PASSWORD)?;
        t.key_passphrase = self.get(&t.id, KEY_PASSPHRASE)?;
        t.proxy_pass = self.get(&t.id, PROXY_PASS)?;
        Ok(())
    }
}

const SECRETS_FILE: &str = "secrets.json";
const KEY_FILE: &str = "secret.key";
const KEY_LEN: usize = 32;
const FILE_VERSION: u32 = 1;

/// 加密文件：密码保存在数据目录的 secrets.json，用同目录 secret.key 里的随机密钥做 AES-256-GCM 加密。
/// 密钥和密码放在一起，只是不让人直接看到明文，能读取数据目录的人都能解出密码。
pub struct FileBackend {
    path: PathBuf,
    key_path: PathBuf,
    key: LessSafeKey,
    state: Mutex<FileState>,
}

struct FileState {
    data: SecretData,
    /// 新生成、还没写进 secret.key 的密钥，第一次保存时写入。
    unsaved_key: Option<[u8; KEY_LEN]>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecretData {
    /// 隧道 ID → 密码类型 → 密码。
    #[serde(default)]
    secrets: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    nonce: String,
    data: String,
}

impl FileBackend {
    /// 读取数据目录下的密码文件。密钥或密码文件损坏时改名留在原处并从空白开始，不影响启动。
    pub fn open(dir: &Path) -> Result<Self, String> {
        let path = dir.join(SECRETS_FILE);
        let key_path = dir.join(KEY_FILE);
        let stored = read_key(&key_path).unwrap_or_else(|e| {
            error!(err = %e, "密钥文件无效");
            move_aside(&key_path);
            None
        });
        let (key_bytes, unsaved_key) = match stored {
            Some(k) => (k, None),
            None => {
                let k = random::<KEY_LEN>()?;
                (k, Some(k))
            }
        };
        let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|_| "密钥长度错误".to_string())?);
        let data = if unsaved_key.is_some() {
            if path.exists() {
                error!("找不到密钥，无法解开已保存的密码");
                move_aside(&path);
            }
            SecretData::default()
        } else {
            read_data(&path, &key).unwrap_or_else(|e| {
                error!(err = %e, "读取密码文件失败");
                move_aside(&path);
                SecretData::default()
            })
        };
        Ok(FileBackend { path, key_path, key, state: Mutex::new(FileState { data, unsaved_key }) })
    }

    /// 修改一份副本并写入文件，写入成功后才替换内存里的数据。
    fn update(&self, f: impl FnOnce(&mut SecretData)) -> Result<(), String> {
        let mut st = self.state.lock().unwrap();
        let mut next = st.data.clone();
        f(&mut next);
        if let Some(k) = st.unsaved_key {
            write_file_atomic(&self.key_path, B64.encode(k).as_bytes())
                .map_err(|e| format!("写入 {} 失败: {e}", self.key_path.display()))?;
            st.unsaved_key = None;
        }
        let plain = serde_json::to_vec(&next).map_err(|e| e.to_string())?;
        let env = seal(&self.key, plain)?;
        let text = serde_json::to_vec_pretty(&env).map_err(|e| e.to_string())?;
        write_file_atomic(&self.path, &text).map_err(|e| format!("写入 {} 失败: {e}", self.path.display()))?;
        st.data = next;
        Ok(())
    }
}

impl SecretBackend for FileBackend {
    fn get(&self, tunnel_id: &str, kind: &str) -> Result<String, String> {
        let st = self.state.lock().unwrap();
        Ok(st.data.secrets.get(tunnel_id).and_then(|m| m.get(kind)).cloned().unwrap_or_default())
    }

    fn set(&self, tunnel_id: &str, kind: &str, value: &str) -> Result<(), String> {
        self.update(|d| {
            d.secrets.entry(tunnel_id.to_string()).or_default().insert(kind.to_string(), value.to_string());
        })
    }

    fn delete(&self, tunnel_id: &str, kind: &str) -> Result<(), String> {
        let exists = {
            let st = self.state.lock().unwrap();
            st.data.secrets.get(tunnel_id).is_some_and(|m| m.contains_key(kind))
        };
        if !exists {
            return Ok(());
        }
        self.update(|d| {
            if let Some(m) = d.secrets.get_mut(tunnel_id) {
                m.remove(kind);
                if m.is_empty() {
                    d.secrets.remove(tunnel_id);
                }
            }
        })
    }
}

fn random<const N: usize>() -> Result<[u8; N], String> {
    let mut buf = [0u8; N];
    SystemRandom::new().fill(&mut buf).map_err(|_| "生成随机数失败".to_string())?;
    Ok(buf)
}

/// 文件不存在时返回 None。
fn read_key(path: &Path) -> Result<Option<[u8; KEY_LEN]>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("读取 {} 失败: {e}", path.display())),
    };
    let bytes = B64.decode(text.trim()).map_err(|e| e.to_string())?;
    let key: [u8; KEY_LEN] = bytes.try_into().map_err(|_| format!("{} 长度不对", path.display()))?;
    Ok(Some(key))
}

fn read_data(path: &Path, key: &LessSafeKey) -> Result<SecretData, String> {
    let text = match std::fs::read(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SecretData::default()),
        Err(e) => return Err(format!("读取 {} 失败: {e}", path.display())),
    };
    let env: Envelope = serde_json::from_slice(&text).map_err(|e| e.to_string())?;
    if env.version != FILE_VERSION {
        return Err(format!("不支持的密码文件版本 {}", env.version));
    }
    let plain = open_sealed(key, &env)?;
    serde_json::from_slice(&plain).map_err(|e| e.to_string())
}

fn seal(key: &LessSafeKey, mut buf: Vec<u8>) -> Result<Envelope, String> {
    let nonce = random::<NONCE_LEN>()?;
    key.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut buf)
        .map_err(|_| "加密失败".to_string())?;
    Ok(Envelope { version: FILE_VERSION, nonce: B64.encode(nonce), data: B64.encode(buf) })
}

fn open_sealed(key: &LessSafeKey, env: &Envelope) -> Result<Vec<u8>, String> {
    let nonce: [u8; NONCE_LEN] = B64
        .decode(&env.nonce)
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|_| "nonce 长度不对".to_string())?;
    let mut buf = B64.decode(&env.data).map_err(|e| e.to_string())?;
    let plain = key
        .open_in_place(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut buf)
        .map_err(|_| "解密失败，密钥和密码文件对不上".to_string())?;
    Ok(plain.to_vec())
}

/// 把无法使用的文件改名为 "原名.broken-时间"，留给用户自己处理。
fn move_aside(path: &Path) {
    if !path.exists() {
        return;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let stamp = chrono::Local::now().format("%Y%m%d%H%M%S");
    let target = path.with_file_name(format!("{name}.broken-{stamp}"));
    match std::fs::rename(path, &target) {
        Ok(()) => warn!(file = %target.display(), "无法使用的文件已改名保留"),
        Err(e) => error!(file = %path.display(), err = %e, "改名失败"),
    }
}

#[derive(Default)]
pub struct MemoryBackend {
    map: Mutex<HashMap<(String, String), String>>,
}

impl SecretBackend for MemoryBackend {
    fn get(&self, tunnel_id: &str, kind: &str) -> Result<String, String> {
        let map = self.map.lock().unwrap();
        Ok(map.get(&(tunnel_id.to_string(), kind.to_string())).cloned().unwrap_or_default())
    }

    fn set(&self, tunnel_id: &str, kind: &str, value: &str) -> Result<(), String> {
        self.map.lock().unwrap().insert((tunnel_id.into(), kind.into()), value.into());
        Ok(())
    }

    fn delete(&self, tunnel_id: &str, kind: &str) -> Result<(), String> {
        self.map.lock().unwrap().remove(&(tunnel_id.to_string(), kind.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("rt-secrets-{}", crate::util::new_id()))
    }

    #[test]
    fn file_backend_persists_encrypted() {
        let dir = temp_dir();
        let b = FileBackend::open(&dir).unwrap();
        assert!(!dir.join(KEY_FILE).exists(), "没有保存过密码时不写文件");
        b.set("t1", PASSWORD, "s3cret-pw").unwrap();
        b.set("t1", PROXY_PASS, "proxy").unwrap();
        b.set("t2", PASSWORD, "other").unwrap();
        b.delete("t2", PASSWORD).unwrap();

        let raw = std::fs::read_to_string(dir.join(SECRETS_FILE)).unwrap();
        assert!(!raw.contains("s3cret-pw"), "文件里没有明文");

        let b = FileBackend::open(&dir).unwrap();
        assert_eq!(b.get("t1", PASSWORD).unwrap(), "s3cret-pw");
        assert_eq!(b.get("t1", PROXY_PASS).unwrap(), "proxy");
        assert_eq!(b.get("t1", KEY_PASSPHRASE).unwrap(), "");
        assert_eq!(b.get("t2", PASSWORD).unwrap(), "");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn wrong_key_moves_file_aside() {
        let dir = temp_dir();
        FileBackend::open(&dir).unwrap().set("t1", PASSWORD, "pw").unwrap();
        std::fs::write(dir.join(KEY_FILE), B64.encode([7u8; KEY_LEN])).unwrap();

        let b = FileBackend::open(&dir).unwrap();
        assert_eq!(b.get("t1", PASSWORD).unwrap(), "");
        assert!(!dir.join(SECRETS_FILE).exists());
        let kept = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("secrets.json.broken-"));
        assert!(kept, "解不开的文件改名保留");
        b.set("t1", PASSWORD, "new").unwrap();
        assert_eq!(FileBackend::open(&dir).unwrap().get("t1", PASSWORD).unwrap(), "new");
        let _ = std::fs::remove_dir_all(dir);
    }
}
