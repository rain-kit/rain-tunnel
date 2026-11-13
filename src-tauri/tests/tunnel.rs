//! 用进程内的 russh 服务器端到端测试隧道：本地 / 远程 / 动态转发、认证、主机指纹、重连。

mod common;

use std::path::PathBuf;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use russh::keys::{Algorithm, PrivateKey};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use common::*;
use rain_tunnel::core::ssh::known_hosts::HostKeyStore;
use rain_tunnel::core::ssh::{HostKeyQuery, TrustFn};
use rain_tunnel::core::tunnel::{Options, Runner};
use rain_tunnel::model::{AuthType, Tunnel, TunnelState, TunnelType};


fn trust_all(count: Arc<AtomicUsize>, accept: bool) -> TrustFn {
    Arc::new(move |_q: HostKeyQuery| {
        count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { accept })
    })
}

struct Env {
    store: Arc<HostKeyStore>,
    prompts: Arc<AtomicUsize>,
    dir: PathBuf,
}

impl Env {
    fn new() -> Env {
        let dir = temp_dir();
        Env {
            store: Arc::new(HostKeyStore::new(dir.join("known_hosts"), vec![])),
            prompts: Arc::new(AtomicUsize::new(0)),
            dir,
        }
    }

    fn opts(&self, accept: bool) -> Options {
        let mut o = Options::new(self.store.clone());
        o.trust = Some(trust_all(self.prompts.clone(), accept));
        o.dial_timeout = Duration::from_secs(5);
        o.wait_connected = Duration::from_secs(3);
        o.min_backoff = Duration::from_millis(100);
        o.max_backoff = Duration::from_millis(300);
        o
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn tunnel(kind: TunnelType, srv: &TestServer) -> Tunnel {
    Tunnel {
        id: "t1".into(),
        name: "test".into(),
        kind,
        ssh_host: "127.0.0.1".into(),
        ssh_port: srv.addr.port() as i32,
        ssh_user: USER.into(),
        auth_type: AuthType::Password,
        password: PASSWORD.into(),
        listen_host: "127.0.0.1".into(),
        listen_port: 0,
        auto_reconnect: true,
        ..Default::default()
    }
}

async fn wait_state(r: &Runner, want: TunnelState) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let st = r.status();
        if st.state == want {
            return;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("等待状态 {want:?} 超时，当前 {:?}：{:?}", st.state, st.error);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn echo_roundtrip(s: &mut TcpStream, msg: &[u8]) {
    s.write_all(msg).await.unwrap();
    let mut buf = vec![0u8; msg.len()];
    tokio::time::timeout(Duration::from_secs(5), s.read_exact(&mut buf)).await.expect("读取超时").unwrap();
    assert_eq!(buf, msg);
}

async fn socks5_connect(proxy: &str, target: SocketAddr, auth: Option<(&str, &str)>) -> std::io::Result<TcpStream> {
    let mut s = TcpStream::connect(proxy).await?;
    match auth {
        None => s.write_all(&[5, 1, 0]).await?,
        Some(_) => s.write_all(&[5, 1, 2]).await?,
    }
    let mut r = [0u8; 2];
    s.read_exact(&mut r).await?;
    if let Some((u, p)) = auth {
        assert_eq!(r, [5, 2]);
        let mut req = vec![1, u.len() as u8];
        req.extend_from_slice(u.as_bytes());
        req.push(p.len() as u8);
        req.extend_from_slice(p.as_bytes());
        s.write_all(&req).await?;
        s.read_exact(&mut r).await?;
        if r[1] != 0 {
            return Err(std::io::Error::other("代理认证失败"));
        }
    } else if r != [5, 0] {
        return Err(std::io::Error::other("代理要求认证"));
    }
    let std::net::IpAddr::V4(ip) = target.ip() else { unreachable!() };
    let mut req = vec![5, 1, 0, 1];
    req.extend_from_slice(&ip.octets());
    req.extend_from_slice(&target.port().to_be_bytes());
    s.write_all(&req).await?;
    let mut rep = [0u8; 10];
    s.read_exact(&mut rep).await?;
    if rep[1] != 0 {
        return Err(std::io::Error::other(format!("SOCKS5 回复 {}", rep[1])));
    }
    Ok(s)
}

// ---------- 测试 ----------

#[tokio::test]
async fn local_forward_and_known_hosts() {
    let srv = TestServer::start().await;
    let echo = echo_server().await;
    let env = Env::new();
    let mut t = tunnel(TunnelType::Local, &srv);
    t.target_host = "127.0.0.1".into();
    t.target_port = echo.port() as i32;

    let r = Runner::new(t.clone(), env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Connected).await;
    let mut c = TcpStream::connect(r.listen_addr()).await.unwrap();
    echo_roundtrip(&mut c, b"hello local").await;
    assert_eq!(r.stats().totals().active, 1);
    assert_eq!(env.prompts.load(Ordering::SeqCst), 1);

    // 停止后已建立的连接被关闭。
    r.stop().await;
    let mut buf = [0u8; 1];
    let n = tokio::time::timeout(Duration::from_secs(3), c.read(&mut buf)).await.expect("停止后连接未关闭");
    assert!(matches!(n, Ok(0) | Err(_)));
    assert_eq!(r.status().state, TunnelState::Stopped);

    // 指纹已保存，再次连接不再询问。
    let r = Runner::new(t, env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Connected).await;
    assert_eq!(env.prompts.load(Ordering::SeqCst), 1);
    r.stop().await;
}

#[tokio::test]
async fn local_connection_waits_for_ssh() {
    let srv = TestServer::start().await;
    let echo = echo_server().await;
    let env = Env::new();
    let mut t = tunnel(TunnelType::Local, &srv);
    t.target_host = "127.0.0.1".into();
    t.target_port = echo.port() as i32;
    let r = Runner::new(t, env.opts(true));
    r.start().await.unwrap();
    // 不等 Connected 直接连，本地监听会等 SSH 连上后再转发。
    let mut c = TcpStream::connect(r.listen_addr()).await.unwrap();
    echo_roundtrip(&mut c, b"early").await;
    r.stop().await;
}

#[tokio::test]
async fn dynamic_socks5_and_http() {
    let srv = TestServer::start().await;
    let echo = echo_server().await;
    let env = Env::new();
    let mut t = tunnel(TunnelType::Dynamic, &srv);
    t.enable_http = true;
    let r = Runner::new(t, env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Connected).await;
    let proxy = r.listen_addr();

    let mut s = socks5_connect(&proxy, echo, None).await.unwrap();
    echo_roundtrip(&mut s, b"via socks5").await;

    let mut h = TcpStream::connect(&proxy).await.unwrap();
    h.write_all(format!("CONNECT {echo} HTTP/1.1\r\nHost: {echo}\r\n\r\n").as_bytes()).await.unwrap();
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut b = [0u8; 1];
        h.read_exact(&mut b).await.unwrap();
        head.push(b[0]);
    }
    assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 200"), "{}", String::from_utf8_lossy(&head));
    echo_roundtrip(&mut h, b"via http connect").await;

    // 目标不可达时返回 SOCKS5 错误而不是挂起。
    let dead = TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap();
    assert!(socks5_connect(&proxy, dead, None).await.is_err());
    r.stop().await;
}

#[tokio::test]
async fn dynamic_proxy_auth() {
    let srv = TestServer::start().await;
    let echo = echo_server().await;
    let env = Env::new();
    let mut t = tunnel(TunnelType::Dynamic, &srv);
    t.proxy_user = "pu".into();
    t.proxy_pass = "pp".into();
    let r = Runner::new(t, env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Connected).await;
    let proxy = r.listen_addr();
    assert!(socks5_connect(&proxy, echo, None).await.is_err());
    assert!(socks5_connect(&proxy, echo, Some(("pu", "bad"))).await.is_err());
    let mut s = socks5_connect(&proxy, echo, Some(("pu", "pp"))).await.unwrap();
    echo_roundtrip(&mut s, b"authed").await;
    r.stop().await;
}

#[tokio::test]
async fn remote_forward() {
    let srv = TestServer::start().await;
    let echo = echo_server().await;
    let env = Env::new();
    let port = TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port();
    let mut t = tunnel(TunnelType::Remote, &srv);
    t.listen_port = port as i32;
    t.target_host = "127.0.0.1".into();
    t.target_port = echo.port() as i32;
    let r = Runner::new(t, env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Connected).await;
    let mut c = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    echo_roundtrip(&mut c, b"hello remote").await;
    r.stop().await;
}

#[tokio::test]
async fn wrong_password_fails_permanently() {
    let srv = TestServer::start().await;
    let env = Env::new();
    let mut t = tunnel(TunnelType::Dynamic, &srv);
    t.password = "wrong".into();
    let r = Runner::new(t, env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Failed).await;
    assert_eq!(r.status().error.map(|e| e.i18n), Some("err.ssh.authDenied"));
    // 失败后本地端口释放。
    TcpListener::bind(r.listen_addr()).await.expect("失败后应释放本地端口");
    r.stop().await;
}

#[tokio::test]
async fn host_key_rejected_and_changed() {
    let srv = TestServer::start().await;
    let env = Env::new();
    let t = tunnel(TunnelType::Dynamic, &srv);

    let r = Runner::new(t.clone(), env.opts(false));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Failed).await;
    r.stop().await;

    // 预先信任另一把密钥，模拟服务器指纹变化。
    let other = PrivateKey::random(&mut russh::keys::key::safe_rng(), Algorithm::Ed25519).unwrap();
    env.store.add("127.0.0.1", srv.addr.port(), other.public_key()).unwrap();
    let before = env.prompts.load(Ordering::SeqCst);
    let r = Runner::new(t, env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Failed).await;
    assert_eq!(r.status().error.map(|e| e.i18n), Some("err.ssh.hostKeyMismatch"));
    assert_eq!(env.prompts.load(Ordering::SeqCst), before, "指纹变化时不应询问用户");
    r.stop().await;
}

#[tokio::test]
async fn reconnects_after_server_disconnect() {
    let srv = TestServer::start().await;
    let echo = echo_server().await;
    let env = Env::new();
    let mut t = tunnel(TunnelType::Local, &srv);
    t.target_host = "127.0.0.1".into();
    t.target_port = echo.port() as i32;
    let r = Runner::new(t, env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Connected).await;
    let addr = r.listen_addr();

    srv.kick_all().await;
    wait_state(&r, TunnelState::Reconnecting).await;
    wait_state(&r, TunnelState::Connected).await;
    assert_eq!(r.listen_addr(), addr, "重连期间本地端口保持不变");
    let mut c = TcpStream::connect(&addr).await.unwrap();
    echo_roundtrip(&mut c, b"after reconnect").await;
    r.stop().await;
}

#[tokio::test]
async fn retries_until_server_comes_back() {
    let key = PrivateKey::random(&mut russh::keys::key::safe_rng(), Algorithm::Ed25519).unwrap();
    let addr = TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap();
    let env = Env::new();
    let mut t = Tunnel { ssh_port: addr.port() as i32, ..Default::default() };
    t.kind = TunnelType::Dynamic;
    t.ssh_host = "127.0.0.1".into();
    t.ssh_user = USER.into();
    t.password = PASSWORD.into();
    t.listen_host = "127.0.0.1".into();
    t.auto_reconnect = true;
    let r = Runner::new(t, env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Reconnecting).await;
    assert!(r.status().attempt >= 1);

    let _srv = TestServer::start_with_key(key, Some(addr)).await;
    wait_state(&r, TunnelState::Connected).await;
    assert_eq!(r.status().attempt, 0);
    r.stop().await;
}

#[tokio::test]
async fn no_auto_reconnect_marks_failed() {
    let srv = TestServer::start().await;
    let env = Env::new();
    let mut t = tunnel(TunnelType::Dynamic, &srv);
    t.auto_reconnect = false;
    let r = Runner::new(t, env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Connected).await;
    srv.kick_all().await;
    wait_state(&r, TunnelState::Failed).await;
    r.stop().await;
}

#[tokio::test]
async fn port_in_use_fails_fast() {
    let srv = TestServer::start().await;
    let env = Env::new();
    let busy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut t = tunnel(TunnelType::Dynamic, &srv);
    t.listen_port = busy.local_addr().unwrap().port() as i32;
    let r = Runner::new(t, env.opts(true));
    let err = r.start().await.unwrap_err();
    assert_eq!(err.i18n, "err.listen.addrInUse");
    assert!(err.text().contains(&busy.local_addr().unwrap().port().to_string()), "{err:?}");
    assert!(!r.running());
}

#[tokio::test]
async fn kill_connection_from_stats() {
    let srv = TestServer::start().await;
    let echo = echo_server().await;
    let env = Env::new();
    let r = Runner::new(tunnel(TunnelType::Dynamic, &srv), env.opts(true));
    r.start().await.unwrap();
    wait_state(&r, TunnelState::Connected).await;
    let mut s = socks5_connect(&r.listen_addr(), echo, None).await.unwrap();
    echo_roundtrip(&mut s, b"x").await;
    let conns = r.stats().active();
    assert_eq!(conns.len(), 1);
    assert_eq!(conns[0].target, echo.to_string());
    assert!(r.stats().kill(conns[0].id));
    let mut buf = [0u8; 1];
    let n = tokio::time::timeout(Duration::from_secs(3), s.read(&mut buf)).await.expect("断开后连接未关闭");
    assert!(matches!(n, Ok(0) | Err(_)));
    r.stop().await;
}
