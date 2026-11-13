use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

/// 拼接 host:port，IPv6 地址加方括号。
pub fn join_host_port(host: &str, port: impl std::fmt::Display) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// 把开头的 ~ 展开为用户主目录。
pub fn expand_home(path: &str) -> PathBuf {
    if path != "~" && !path.starts_with("~/") && !path.starts_with("~\\") {
        return PathBuf::from(path);
    }
    match dirs::home_dir() {
        Some(home) if path.len() > 2 => home.join(&path[2..]),
        Some(home) => home,
        None => PathBuf::from(path),
    }
}

/// 先写临时文件再重命名，避免写到一半崩溃导致文件损坏。
pub fn write_file_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!(".{name}-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
        drop(f);
        restrict_permissions(&tmp);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(unix)]
pub fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
pub fn restrict_permissions(_path: &Path) {}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub fn is_loopback(host: &str) -> bool {
    host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_port() {
        assert_eq!(join_host_port("example.com", 22), "example.com:22");
        assert_eq!(join_host_port("::1", 1080), "[::1]:1080");
    }

    #[test]
    fn loopback() {
        assert!(is_loopback("127.0.0.1"));
        assert!(is_loopback("127.1.2.3"));
        assert!(is_loopback("::1"));
        assert!(is_loopback("localhost"));
        assert!(!is_loopback("0.0.0.0"));
        assert!(!is_loopback("192.168.1.1"));
    }
}
