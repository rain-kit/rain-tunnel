use std::io::{self, Write};
use std::path::{Path, PathBuf};

use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use russh::keys::{Algorithm, HashAlg, PublicKey};
use sha1::Sha1;
use tokio::sync::{Mutex, MutexGuard};

use crate::util::restrict_permissions;

pub enum Check {
    Known,
    /// 本机没有记录过这台主机。
    Unknown,
    /// 主机已记录但密钥不同，附带已记录密钥的指纹。
    Mismatch(Vec<String>),
}

/// 管理应用自己的 known_hosts 文件；同时只读地参考用户的 ~/.ssh/known_hosts。
pub struct HostKeyStore {
    path: PathBuf,
    extra: Vec<PathBuf>,
    /// 校验和询问用户期间持有，避免多条隧道同时连接同一台新主机时重复询问。
    lock: Mutex<()>,
}

impl HostKeyStore {
    pub fn new(path: PathBuf, extra: Vec<PathBuf>) -> Self {
        HostKeyStore { path, extra, lock: Mutex::new(()) }
    }

    /// ~/.ssh/known_hosts（存在时）。
    pub fn user_known_hosts() -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|h| h.join(".ssh").join("known_hosts"))
            .filter(|p| p.is_file())
            .into_iter()
            .collect()
    }

    pub async fn lock(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().await
    }

    fn keys_for(&self, host: &str, port: u16) -> io::Result<Vec<PublicKey>> {
        let name = normalize(host, port);
        let mut keys = Vec::new();
        match std::fs::read_to_string(&self.path) {
            Ok(text) => keys.extend(parse_matching(&text, &name)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        for p in &self.extra {
            if let Ok(text) = std::fs::read_to_string(p) {
                keys.extend(parse_matching(&text, &name));
            }
        }
        Ok(keys)
    }

    pub fn check(&self, host: &str, port: u16, key: &PublicKey) -> io::Result<Check> {
        let known = self.keys_for(host, port)?;
        if known.is_empty() {
            return Ok(Check::Unknown);
        }
        if known.iter().any(|k| k.key_data() == key.key_data()) {
            return Ok(Check::Known);
        }
        Ok(Check::Mismatch(known.iter().map(fingerprint).collect()))
    }

    /// 已记录密钥对应的主机密钥算法，用于握手时优先协商这些算法，
    /// 避免服务器同时有多种密钥时因算法不同被误判为指纹变更。主机未知时返回空。
    pub fn known_algorithms(&self, host: &str, port: u16) -> Vec<Algorithm> {
        let mut out: Vec<Algorithm> = Vec::new();
        for k in self.keys_for(host, port).unwrap_or_default() {
            let algos = match k.algorithm() {
                Algorithm::Rsa { .. } => vec![
                    Algorithm::Rsa { hash: Some(HashAlg::Sha512) },
                    Algorithm::Rsa { hash: Some(HashAlg::Sha256) },
                    Algorithm::Rsa { hash: None },
                ],
                a @ (Algorithm::Ed25519 | Algorithm::Ecdsa { .. }) => vec![a],
                _ => vec![],
            };
            for a in algos {
                if !out.contains(&a) {
                    out.push(a);
                }
            }
        }
        out
    }

    pub fn add(&self, host: &str, port: u16, key: &PublicKey) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let needs_newline = std::fs::read(&self.path).is_ok_and(|d| !d.is_empty() && !d.ends_with(b"\n"));
        let mut line = String::new();
        if needs_newline {
            line.push('\n');
        }
        let mut key = key.clone();
        key.set_comment("");
        let encoded = key.to_openssh().map_err(io::Error::other)?;
        line.push_str(&format!("{} {}\n", normalize(host, port), encoded.trim()));
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        f.write_all(line.as_bytes())?;
        restrict_permissions(&self.path);
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// known_hosts 中的主机名写法：22 端口直接写主机名，其他端口写 [host]:port。
pub fn normalize(host: &str, port: u16) -> String {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}

fn parse_matching(text: &str, name: &str) -> Vec<PublicKey> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('@') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let (Some(hosts), Some(kind), Some(data)) = (fields.next(), fields.next(), fields.next()) else {
            continue;
        };
        if !hosts_match(hosts, name) {
            continue;
        }
        if let Ok(k) = PublicKey::from_openssh(&format!("{kind} {data}")) {
            out.push(k);
        }
    }
    out
}

fn hosts_match(field: &str, name: &str) -> bool {
    if let Some(hashed) = field.strip_prefix("|1|") {
        return hashed_match(hashed, name);
    }
    let name = name.to_ascii_lowercase();
    let mut matched = false;
    for pat in field.split(',') {
        let pat = pat.to_ascii_lowercase();
        if let Some(neg) = pat.strip_prefix('!') {
            if wildcard_match(neg.as_bytes(), name.as_bytes()) {
                return false;
            }
        } else if wildcard_match(pat.as_bytes(), name.as_bytes()) {
            matched = true;
        }
    }
    matched
}

fn hashed_match(hashed: &str, name: &str) -> bool {
    let b64 = base64::engine::general_purpose::STANDARD;
    let Some((salt, hash)) = hashed.split_once('|') else { return false };
    let (Ok(salt), Ok(hash)) = (b64.decode(salt), b64.decode(hash)) else { return false };
    let Ok(mut mac) = <Hmac<Sha1> as KeyInit>::new_from_slice(&salt) else { return false };
    mac.update(name.as_bytes());
    mac.verify_slice(&hash).is_ok()
}

/// 支持 * 和 ? 的通配匹配。
fn wildcard_match(pat: &[u8], s: &[u8]) -> bool {
    let (mut p, mut i) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while i < s.len() {
        if p < pat.len() && (pat[p] == b'?' || pat[p] == s[i]) {
            p += 1;
            i += 1;
        } else if p < pat.len() && pat[p] == b'*' {
            star = Some(p);
            mark = i;
            p += 1;
        } else if let Some(sp) = star {
            p = sp + 1;
            mark += 1;
            i = mark;
        } else {
            return false;
        }
    }
    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl";

    #[test]
    fn patterns() {
        assert!(hosts_match("example.com,10.0.0.1", "example.com"));
        assert!(hosts_match("*.example.com", "a.example.com"));
        assert!(!hosts_match("*.example.com,!bad.example.com", "bad.example.com"));
        assert!(hosts_match("[example.com]:2222", "[example.com]:2222"));
        assert!(!hosts_match("example.com", "[example.com]:2222"));
        assert!(hosts_match("h?st", "host"));
        assert!(hosts_match("EXAMPLE.com", "example.COM"));
    }

    #[test]
    fn hashed_host() {
        // ssh-keygen -H 生成的 example.com 条目
        let salt = base64::engine::general_purpose::STANDARD.encode(b"0123456789abcdefghij");
        let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(b"0123456789abcdefghij").unwrap();
        mac.update(b"example.com");
        let hash = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
        let field = format!("|1|{salt}|{hash}");
        assert!(hosts_match(&field, "example.com"));
        assert!(!hosts_match(&field, "other.com"));
    }

    #[test]
    fn check_and_add() {
        let dir = std::env::temp_dir().join(format!("st-kh-{}", crate::util::new_id()));
        let store = HostKeyStore::new(dir.join("known_hosts"), vec![]);
        let key = PublicKey::from_openssh(ED).unwrap();
        assert!(matches!(store.check("h", 2222, &key).unwrap(), Check::Unknown));
        store.add("h", 2222, &key).unwrap();
        assert!(matches!(store.check("h", 2222, &key).unwrap(), Check::Known));
        assert!(matches!(store.check("h", 22, &key).unwrap(), Check::Unknown));
        assert_eq!(store.known_algorithms("h", 2222), vec![Algorithm::Ed25519]);

        let other = russh::keys::PrivateKey::random(&mut rand_core(), Algorithm::Ed25519).unwrap();
        match store.check("h", 2222, other.public_key()).unwrap() {
            Check::Mismatch(fps) => assert_eq!(fps, vec![fingerprint(&key)]),
            _ => panic!("应识别为指纹变更"),
        }
        let text = std::fs::read_to_string(dir.join("known_hosts")).unwrap();
        assert!(text.starts_with("[h]:2222 ssh-ed25519 "));
        let _ = std::fs::remove_dir_all(dir);
    }

    fn rand_core() -> impl russh::keys::signature::rand_core::CryptoRng {
        russh::keys::key::safe_rng()
    }
}
