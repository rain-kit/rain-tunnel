use std::collections::HashMap;
use std::sync::Mutex;

use crate::config::KEYRING_PREFIX;
use crate::model::Tunnel;

pub const PASSWORD: &str = "password";
pub const KEY_PASSPHRASE: &str = "keyPassphrase";
pub const PROXY_PASS: &str = "proxyPass";
const KINDS: [&str; 3] = [PASSWORD, KEY_PASSPHRASE, PROXY_PASS];

/// 密码存储后端：正式运行用系统凭据库，测试用内存实现。
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

    /// 把凭据库中的密码填回隧道定义，用于启动隧道。
    pub fn fill(&self, t: &mut Tunnel) -> Result<(), String> {
        t.password = self.get(&t.id, PASSWORD)?;
        t.key_passphrase = self.get(&t.id, KEY_PASSPHRASE)?;
        t.proxy_pass = self.get(&t.id, PROXY_PASS)?;
        Ok(())
    }
}

/// 系统凭据库（Windows 凭据管理器 / macOS 钥匙串 / Linux Secret Service）。
/// 条目命名与 v1（go-keyring）一致：服务名 "ssh-tunnel:<隧道ID>"，账户名为密码类型，
/// Windows 上凭据名为 "服务名:账户名"，内容为 UTF-8，因此 v2 可以直接读取 v1 保存的密码。
pub struct KeyringBackend;

impl KeyringBackend {
    fn entry(tunnel_id: &str, kind: &str) -> keyring::Result<keyring::Entry> {
        let service = format!("{KEYRING_PREFIX}:{tunnel_id}");
        if cfg!(windows) {
            keyring::Entry::new_with_target(&format!("{service}:{kind}"), &service, kind)
        } else {
            keyring::Entry::new(&service, kind)
        }
    }
}

impl SecretBackend for KeyringBackend {
    fn get(&self, tunnel_id: &str, kind: &str) -> Result<String, String> {
        let entry = Self::entry(tunnel_id, kind).map_err(|e| e.to_string())?;
        match entry.get_secret() {
            Ok(bytes) => Ok(String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())),
            Err(keyring::Error::NoEntry) => Ok(String::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, tunnel_id: &str, kind: &str, value: &str) -> Result<(), String> {
        let entry = Self::entry(tunnel_id, kind).map_err(|e| e.to_string())?;
        entry.set_secret(value.as_bytes()).map_err(|e| e.to_string())
    }

    fn delete(&self, tunnel_id: &str, kind: &str) -> Result<(), String> {
        let entry = Self::entry(tunnel_id, kind).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
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
