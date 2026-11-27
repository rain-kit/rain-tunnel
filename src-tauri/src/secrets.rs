use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use keyring_core::{CredentialStore, Entry, Error as KeyringError};

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
/// Linux 上不指定 target，按服务名和账户名查找，v1 和 keyring v3 保存的条目都能找到。
#[derive(Default)]
pub struct KeyringBackend {
    store: OnceLock<Result<Arc<CredentialStore>, String>>,
}

impl KeyringBackend {
    fn store(&self) -> Result<&Arc<CredentialStore>, String> {
        self.store.get_or_init(|| open_store().map_err(|e| e.to_string())).as_ref().map_err(Clone::clone)
    }

    fn entry(&self, tunnel_id: &str, kind: &str) -> Result<Entry, String> {
        let store = self.store()?;
        let service = format!("{KEYRING_PREFIX}:{tunnel_id}");
        let entry = if cfg!(windows) {
            let target = format!("{service}:{kind}");
            Entry::new_in_store_with_modifiers(store, &service, kind, &HashMap::from([("target", target.as_str())]))
        } else {
            Entry::new_in_store(store, &service, kind)
        };
        entry.map_err(|e| e.to_string())
    }
}

#[cfg(windows)]
fn open_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Ok(windows_native_keyring_store::Store::new()?)
}

#[cfg(target_os = "macos")]
fn open_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Ok(apple_native_keyring_store::keychain::Store::new()?)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Ok(dbus_secret_service_keyring_store::Store::new()?)
}

impl SecretBackend for KeyringBackend {
    fn get(&self, tunnel_id: &str, kind: &str) -> Result<String, String> {
        match self.entry(tunnel_id, kind)?.get_secret() {
            Ok(bytes) => Ok(String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())),
            Err(KeyringError::NoEntry) => Ok(String::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, tunnel_id: &str, kind: &str, value: &str) -> Result<(), String> {
        self.entry(tunnel_id, kind)?.set_secret(value.as_bytes()).map_err(|e| e.to_string())
    }

    fn delete(&self, tunnel_id: &str, kind: &str) -> Result<(), String> {
        match self.entry(tunnel_id, kind)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
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
