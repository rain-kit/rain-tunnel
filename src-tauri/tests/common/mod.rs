//! 集成测试共用的进程内 SSH 服务器和回显服务器。
#![allow(dead_code)]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::keys::{Algorithm, PrivateKey};
use russh::server::{self, Auth, ChannelOpenHandle, Msg, Session};
use russh::{Channel, Disconnect};
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

pub const USER: &str = "tester";
pub const PASSWORD: &str = "pa55";

// ---------- 测试用 SSH 服务器 ----------

pub struct TestServer {
    pub addr: SocketAddr,
    pub key: PrivateKey,
    handles: Arc<Mutex<Vec<server::Handle>>>,
}

impl TestServer {
    pub async fn start() -> TestServer {
        let key = PrivateKey::random(&mut russh::keys::key::safe_rng(), Algorithm::Ed25519).unwrap();
        Self::start_with_key(key, None).await
    }

    pub async fn start_with_key(key: PrivateKey, addr: Option<SocketAddr>) -> TestServer {
        let cfg = Arc::new(server::Config {
            keys: vec![key.clone()],
            auth_rejection_time: Duration::from_millis(10),
            auth_rejection_time_initial: Some(Duration::ZERO),
            ..Default::default()
        });
        let ln = TcpListener::bind(addr.unwrap_or_else(|| "127.0.0.1:0".parse().unwrap())).await.unwrap();
        let addr = ln.local_addr().unwrap();
        let handles = Arc::new(Mutex::new(Vec::new()));
        let hs = handles.clone();
        tokio::spawn(async move {
            while let Ok((tcp, _)) = ln.accept().await {
                let (cfg, hs) = (cfg.clone(), hs.clone());
                tokio::spawn(async move {
                    let h = ServerHandler { token: CancellationToken::new() };
                    if let Ok(rs) = server::run_stream(cfg, tcp, h).await {
                        hs.lock().unwrap().push(rs.handle());
                        let _ = rs.await;
                    }
                });
            }
        });
        TestServer { addr, key, handles }
    }

    /// 模拟服务器主动断开所有会话。
    pub async fn kick_all(&self) {
        let hs: Vec<_> = self.handles.lock().unwrap().drain(..).collect();
        for h in hs {
            let _ = h.disconnect(Disconnect::ByApplication, "bye".into(), String::new()).await;
        }
    }
}

struct ServerHandler {
    token: CancellationToken,
}

impl Drop for ServerHandler {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

impl server::Handler for ServerHandler {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if user == USER && password == PASSWORD { Auth::Accept } else { Auth::reject() })
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        match TcpStream::connect((host, port as u16)).await {
            Ok(mut tcp) => {
                reply.accept().await;
                tokio::spawn(async move {
                    let mut ch = channel.into_stream();
                    let _ = copy_bidirectional(&mut tcp, &mut ch).await;
                });
            }
            Err(_) => reply.reject(russh::ChannelOpenFailure::ConnectFailed).await,
        }
        Ok(())
    }

    async fn tcpip_forward(&mut self, address: &str, port: &mut u32, session: &mut Session) -> Result<bool, Self::Error> {
        let Ok(ln) = TcpListener::bind((address, *port as u16)).await else { return Ok(false) };
        if *port == 0 {
            *port = ln.local_addr().unwrap().port() as u32;
        }
        let handle = session.handle();
        let (addr, p, token) = (address.to_string(), *port, self.token.clone());
        tokio::spawn(async move {
            loop {
                let (mut s, peer) = tokio::select! {
                    _ = token.cancelled() => break,
                    r = ln.accept() => match r { Ok(x) => x, Err(_) => break },
                };
                let Ok(ch) = handle
                    .channel_open_forwarded_tcpip(addr.clone(), p, peer.ip().to_string(), peer.port() as u32)
                    .await
                else {
                    break;
                };
                tokio::spawn(async move {
                    let mut cs = ch.into_stream();
                    let _ = copy_bidirectional(&mut s, &mut cs).await;
                });
            }
        });
        Ok(true)
    }
}

// ---------- 工具 ----------

pub async fn echo_server() -> SocketAddr {
    let ln = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = ln.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = ln.accept().await {
            tokio::spawn(async move {
                let (mut r, mut w) = s.split();
                let _ = tokio::io::copy(&mut r, &mut w).await;
            });
        }
    });
    addr
}

pub fn temp_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("raintunnel-it-{}", rain_tunnel::util::new_id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}
