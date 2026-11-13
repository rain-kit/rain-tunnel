use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::Credentials;

pub const VERSION: u8 = 0x05;

const METHOD_NO_AUTH: u8 = 0x00;
const METHOD_USER_PASS: u8 = 0x02;
const METHOD_NO_ACCEPTABLE: u8 = 0xFF;
const USER_PASS_VERSION: u8 = 0x01;
const CMD_CONNECT: u8 = 0x01;
const ATYP_IPV4: u8 = 0x01;
const ATYP_DOMAIN: u8 = 0x03;
const ATYP_IPV6: u8 = 0x04;

pub const REP_SUCCEEDED: u8 = 0x00;
pub const REP_GENERAL_FAILURE: u8 = 0x01;
pub const REP_NOT_ALLOWED: u8 = 0x02;
pub const REP_HOST_UNREACHABLE: u8 = 0x04;
pub const REP_CONNECTION_REFUSED: u8 = 0x05;
pub const REP_COMMAND_NOT_SUPPORTED: u8 = 0x07;
pub const REP_ADDR_NOT_SUPPORTED: u8 = 0x08;

#[derive(Debug, thiserror::Error)]
pub enum Socks5Error {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("客户端没有提供可接受的认证方式")]
    NoAcceptableMethod,
    #[error("用户名或密码错误")]
    BadCredentials,
    #[error("版本号错误: {0}")]
    BadVersion(u8),
    /// 需要先回复客户端错误码再关闭连接。
    #[error("{msg}")]
    Reply { code: u8, msg: String },
}

/// 完成方法协商和（可选的）RFC 1929 用户名密码认证。
pub async fn negotiate<S>(c: &mut S, auth: Option<&Credentials>) -> Result<(), Socks5Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut hdr = [0u8; 2];
    c.read_exact(&mut hdr).await?;
    if hdr[0] != VERSION {
        return Err(Socks5Error::BadVersion(hdr[0]));
    }
    let mut methods = vec![0u8; hdr[1] as usize];
    c.read_exact(&mut methods).await?;

    let want = if auth.is_some() { METHOD_USER_PASS } else { METHOD_NO_AUTH };
    if !methods.contains(&want) {
        let _ = c.write_all(&[VERSION, METHOD_NO_ACCEPTABLE]).await;
        return Err(Socks5Error::NoAcceptableMethod);
    }
    c.write_all(&[VERSION, want]).await?;
    let Some(auth) = auth else { return Ok(()) };

    let mut ver = [0u8; 1];
    c.read_exact(&mut ver).await?;
    let user = read_len_prefixed(c).await?;
    let pass = read_len_prefixed(c).await?;
    if ver[0] != USER_PASS_VERSION || !auth.matches(&user, &pass) {
        let _ = c.write_all(&[USER_PASS_VERSION, 0x01]).await;
        return Err(Socks5Error::BadCredentials);
    }
    c.write_all(&[USER_PASS_VERSION, 0x00]).await?;
    Ok(())
}

async fn read_len_prefixed<S: AsyncRead + Unpin>(c: &mut S) -> io::Result<Vec<u8>> {
    let mut n = [0u8; 1];
    c.read_exact(&mut n).await?;
    let mut b = vec![0u8; n[0] as usize];
    c.read_exact(&mut b).await?;
    Ok(b)
}

/// 读取 CONNECT 请求，返回目标主机和端口。
pub async fn read_request<S: AsyncRead + Unpin>(c: &mut S) -> Result<(String, u16), Socks5Error> {
    let mut hdr = [0u8; 4];
    c.read_exact(&mut hdr).await?;
    if hdr[0] != VERSION {
        return Err(Socks5Error::BadVersion(hdr[0]));
    }
    if hdr[1] != CMD_CONNECT {
        // SSH 的 direct-tcpip 只能转发 TCP，BIND 和 UDP ASSOCIATE 都不支持。
        return Err(Socks5Error::Reply { code: REP_COMMAND_NOT_SUPPORTED, msg: format!("不支持的命令: {}", hdr[1]) });
    }
    let host = match hdr[3] {
        ATYP_IPV4 => {
            let mut ip = [0u8; 4];
            c.read_exact(&mut ip).await?;
            Ipv4Addr::from(ip).to_string()
        }
        ATYP_IPV6 => {
            let mut ip = [0u8; 16];
            c.read_exact(&mut ip).await?;
            Ipv6Addr::from(ip).to_string()
        }
        ATYP_DOMAIN => {
            let name = read_len_prefixed(c).await?;
            if name.is_empty() {
                return Err(Socks5Error::Reply { code: REP_ADDR_NOT_SUPPORTED, msg: "域名为空".into() });
            }
            String::from_utf8_lossy(&name).into_owned()
        }
        other => {
            return Err(Socks5Error::Reply { code: REP_ADDR_NOT_SUPPORTED, msg: format!("不支持的地址类型: {other}") });
        }
    };
    let mut port = [0u8; 2];
    c.read_exact(&mut port).await?;
    Ok((host, u16::from_be_bytes(port)))
}

pub async fn write_reply<S: AsyncWrite + Unpin>(c: &mut S, code: u8) -> io::Result<()> {
    c.write_all(&[VERSION, code, 0x00, ATYP_IPV4, 0, 0, 0, 0, 0, 0]).await?;
    c.flush().await
}
