use std::io;

use base64::Engine;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

use super::Credentials;

const MAX_HEAD: usize = 64 * 1024;

/// 逐跳头部只对客户端到代理这一段有效，转发前必须去掉。
/// Transfer-Encoding 保留：请求体按原样透传，分块编码要原样交给上游解析。
const HOP_HEADERS: [&str; 7] = [
    "connection",
    "proxy-connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "upgrade",
];

#[derive(Debug)]
pub struct RequestHead {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
}

impl RequestHead {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

pub fn is_method_start(b: u8) -> bool {
    b.is_ascii_uppercase()
}

/// 读取请求行和头部（到空行为止），不读请求体。
pub async fn read_head<R: AsyncBufRead + Unpin>(r: &mut R) -> io::Result<RequestHead> {
    let mut total = 0usize;
    let mut lines = Vec::new();
    loop {
        let mut line = Vec::new();
        let n = r.read_until(b'\n', &mut line).await?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        total += n;
        if total > MAX_HEAD {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "请求头过大"));
        }
        while matches!(line.last(), Some(b'\n' | b'\r')) {
            line.pop();
        }
        if line.is_empty() {
            if lines.is_empty() {
                continue;
            }
            break;
        }
        lines.push(String::from_utf8_lossy(&line).into_owned());
    }
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "HTTP 请求格式错误");
    let mut parts = lines[0].split_whitespace();
    let (method, uri, version) = match (parts.next(), parts.next(), parts.next()) {
        (Some(m), Some(u), Some(v)) if v.starts_with("HTTP/") => (m.to_string(), u.to_string(), v.to_string()),
        _ => return Err(bad()),
    };
    let mut headers = Vec::with_capacity(lines.len() - 1);
    for l in &lines[1..] {
        let (k, v) = l.split_once(':').ok_or_else(bad)?;
        headers.push((k.trim().to_string(), v.trim().to_string()));
    }
    Ok(RequestHead { method, uri, version, headers })
}

pub fn check_auth(head: &RequestHead, auth: &Credentials) -> bool {
    let Some(h) = head.header("Proxy-Authorization") else { return false };
    let Some((scheme, encoded)) = h.split_once(' ') else { return false };
    if !scheme.eq_ignore_ascii_case("Basic") {
        return false;
    }
    let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(encoded.trim()) else { return false };
    let Some(i) = raw.iter().position(|&b| b == b':') else { return false };
    auth.matches(&raw[..i], &raw[i + 1..])
}

/// 解析 host:port / [v6]:port，缺省端口时补上 default_port。
pub fn split_host_port(s: &str, default_port: u16) -> Option<(String, u16)> {
    if let Some(rest) = s.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(p) => p.parse().ok()?,
            None if after.is_empty() => default_port,
            None => return None,
        };
        return Some((host.to_string(), port));
    }
    match s.rsplit_once(':') {
        // 多个冒号且没有方括号，视为不带端口的 IPv6 地址。
        Some((host, _)) if host.contains(':') => Some((s.to_string(), default_port)),
        Some((host, port)) => Some((host.to_string(), port.parse().ok()?)),
        None if s.is_empty() => None,
        None => Some((s.to_string(), default_port)),
    }
}

/// 把绝对地址 http://host[:port]/path 拆成目标地址和 origin-form 路径。
pub fn split_absolute_uri(uri: &str) -> Option<(String, u16, String, String)> {
    let scheme_end = uri.find("://")?;
    if !uri[..scheme_end].eq_ignore_ascii_case("http") {
        return None;
    }
    let rest = &uri[scheme_end + 3..];
    let path_start = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let mut authority = &rest[..path_start];
    if let Some(i) = authority.rfind('@') {
        authority = &authority[i + 1..];
    }
    let (host, port) = split_host_port(authority, 80)?;
    if host.is_empty() {
        return None;
    }
    let mut path = rest[path_start..].split('#').next().unwrap_or("").to_string();
    if !path.starts_with('/') {
        path.insert(0, '/');
    }
    Some((host, port, path, authority.to_string()))
}

/// 生成发给上游的请求头：origin-form 路径、去掉逐跳头部、要求上游响应后关闭连接。
pub fn rewrite_head(head: &RequestHead, path: &str, authority: &str) -> Vec<u8> {
    let listed: Vec<String> = head
        .header("Connection")
        .map(|v| v.split(',').map(|s| s.trim().to_ascii_lowercase()).collect())
        .unwrap_or_default();
    let mut out = format!("{} {} {}\r\n", head.method, path, head.version);
    let mut has_host = false;
    for (k, v) in &head.headers {
        let lower = k.to_ascii_lowercase();
        if HOP_HEADERS.contains(&lower.as_str()) || listed.contains(&lower) {
            continue;
        }
        has_host |= lower == "host";
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    if !has_host {
        out.push_str(&format!("Host: {authority}\r\n"));
    }
    out.push_str("Connection: close\r\n\r\n");
    out.into_bytes()
}

pub async fn write_error<W: AsyncWrite + Unpin>(w: &mut W, code: u16, reason: &str, body: &str) -> io::Result<()> {
    let resp = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    w.write_all(resp.as_bytes()).await?;
    w.flush().await
}

pub const AUTH_REQUIRED: &str = "HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"rain-tunnel\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
pub const CONNECT_OK: &str = "HTTP/1.1 200 Connection Established\r\n\r\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_port_parsing() {
        assert_eq!(split_host_port("example.com", 443), Some(("example.com".into(), 443)));
        assert_eq!(split_host_port("example.com:8443", 443), Some(("example.com".into(), 8443)));
        assert_eq!(split_host_port("[::1]:80", 443), Some(("::1".into(), 80)));
        assert_eq!(split_host_port("[::1]", 443), Some(("::1".into(), 443)));
        assert_eq!(split_host_port("::1", 443), Some(("::1".into(), 443)));
        assert_eq!(split_host_port("h:x", 443), None);
    }

    #[test]
    fn absolute_uri() {
        assert_eq!(
            split_absolute_uri("http://example.com/a?b=1"),
            Some(("example.com".into(), 80, "/a?b=1".into(), "example.com".into()))
        );
        assert_eq!(
            split_absolute_uri("HTTP://u:p@[::1]:8080?x"),
            Some(("::1".into(), 8080, "/?x".into(), "[::1]:8080".into()))
        );
        assert!(split_absolute_uri("https://example.com/").is_none());
        assert!(split_absolute_uri("/relative").is_none());
    }

    #[tokio::test]
    async fn parse_and_rewrite() {
        let raw = b"GET http://example.com/x HTTP/1.1\r\nHost: example.com\r\nProxy-Connection: keep-alive\r\nConnection: X-Foo\r\nX-Foo: 1\r\nAccept: */*\r\n\r\nBODY";
        let mut r = tokio::io::BufReader::new(&raw[..]);
        let head = read_head(&mut r).await.unwrap();
        assert_eq!(head.method, "GET");
        let (_, _, path, authority) = split_absolute_uri(&head.uri).unwrap();
        let out = String::from_utf8(rewrite_head(&head, &path, &authority)).unwrap();
        assert_eq!(out, "GET /x HTTP/1.1\r\nHost: example.com\r\nAccept: */*\r\nConnection: close\r\n\r\n");
    }
}
