pub mod auth;
pub mod known_hosts;

use std::borrow::Cow;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use russh::client::{self, DisconnectReason, Handle, Session};
use russh::keys::PublicKeyOrCertificate;
use russh::{Channel, ChannelOpenFailure};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tokio_util::sync::{CancellationToken, WaitForCancellationFutureOwned};

use crate::core::forward::DialError;
use crate::i18n::Msg;
use crate::model::Tunnel;
use crate::util::join_host_port;
use known_hosts::{Check, HostKeyStore, fingerprint, normalize};

#[derive(Debug, thiserror::Error)]
pub enum SshError {
    /// 重试也不会成功（认证失败、私钥错误等），隧道遇到它应停止重连。
    #[error("{}", .0.text())]
    Permanent(Msg),
    #[error("用户拒绝信任该主机指纹")]
    HostKeyRejected,
    #[error("{}", mismatch(.host, .got, .expected).text())]
    HostKeyMismatch { host: String, got: String, expected: String },
    #[error("已取消")]
    Cancelled,
    #[error("{}", .0.text())]
    Other(Msg),
    #[error("{}", describe(.0).text())]
    Russh(#[from] russh::Error),
}

impl SshError {
    pub fn is_permanent(&self) -> bool {
        matches!(self, SshError::Permanent(_) | SshError::HostKeyRejected | SshError::HostKeyMismatch { .. })
    }

    /// 返回给前端的消息。
    pub fn msg(&self) -> Msg {
        match self {
            SshError::Permanent(m) | SshError::Other(m) => m.clone(),
            SshError::HostKeyRejected => msg!("err.ssh.hostKeyRejected", "用户拒绝信任该主机指纹"),
            SshError::HostKeyMismatch { host, got, expected } => mismatch(host, got, expected),
            SshError::Cancelled => msg!("err.ssh.cancelled", "已取消"),
            SshError::Russh(e) => describe(e),
        }
    }
}

fn mismatch(host: &str, got: &str, expected: &str) -> Msg {
    msg!(
        "err.ssh.hostKeyMismatch",
        "主机 {host} 的指纹与已记录的不一致（收到 {got}，记录为 {expected}），可能存在中间人攻击",
        host,
        got,
        expected
    )
}

fn describe(e: &russh::Error) -> Msg {
    use russh::Error as E;
    match e {
        E::KeepaliveTimeout => msg!("err.ssh.keepaliveTimeout", "连续多个周期保活无响应，连接已断开"),
        E::HUP | E::Disconnect => msg!("err.ssh.serverClosed", "服务器关闭了连接"),
        E::ConnectionTimeout => msg!("err.ssh.timeout", "连接超时"),
        E::IO(io) => msg!("err.ssh.connectionLost", "连接断开").with_cause(io),
        E::NoCommonAlgo { kind, .. } => {
            msg!("err.ssh.noCommonAlgo", "与服务器没有共同支持的 {kind} 算法", kind = format!("{kind:?}"))
        }
        E::Version => msg!("err.ssh.notSshServer", "对方不是 SSH 服务器（版本标识无效）"),
        other => msg!("err.ssh.other", "SSH 错误").with_cause(other),
    }
}

/// 遇到未知主机时询问用户是否信任；cancel 取消时应尽快返回 false。
pub struct HostKeyQuery {
    pub host: String,
    pub key_type: String,
    pub fingerprint: String,
    pub cancel: CancellationToken,
}

pub type TrustFn = Arc<dyn Fn(HostKeyQuery) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

pub struct DialOptions {
    pub host_keys: Arc<HostKeyStore>,
    pub trust: Option<TrustFn>,
    pub timeout: Duration,
    pub keepalive: Option<Duration>,
    pub keepalive_max: usize,
}

/// 服务器为远程转发开过来的连接。
pub struct Forwarded {
    pub channel: Channel<client::Msg>,
    pub origin: String,
}

/// 握手超时计时器：等用户确认指纹的时间不计入。
#[derive(Default)]
struct PauseClock {
    state: Mutex<(Duration, Option<Instant>)>,
}

impl PauseClock {
    fn pause(&self) {
        self.state.lock().unwrap().1 = Some(Instant::now());
    }

    fn resume(&self) {
        let mut s = self.state.lock().unwrap();
        if let Some(since) = s.1.take() {
            s.0 += since.elapsed();
        }
    }

    async fn expired(&self, start: Instant, timeout: Duration) {
        loop {
            let (paused, since) = *self.state.lock().unwrap();
            if since.is_some() {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
            let elapsed = start.elapsed().saturating_sub(paused);
            if elapsed >= timeout {
                return;
            }
            tokio::time::sleep(timeout - elapsed).await;
        }
    }
}

pub struct ClientHandler {
    host: String,
    port: u16,
    store: Arc<HostKeyStore>,
    trust: Option<TrustFn>,
    cancel: CancellationToken,
    clock: Arc<PauseClock>,
    disconnect_tx: Option<oneshot::Sender<Msg>>,
    forwarded_tx: mpsc::UnboundedSender<Forwarded>,
}

impl client::Handler for ClientHandler {
    type Error = SshError;

    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool, SshError> {
        let key = key.public_key();
        let _guard = self.store.lock().await;
        let name = normalize(&self.host, self.port);
        match self.store.check(&self.host, self.port, &key) {
            Err(e) => Err(SshError::Permanent(msg!("err.ssh.knownHostsReadFailed", "读取 known_hosts 失败").with_cause(e))),
            Ok(Check::Known) => Ok(true),
            Ok(Check::Mismatch(expected)) => Err(SshError::HostKeyMismatch {
                host: name,
                got: fingerprint(&key),
                expected: expected.join(", "),
            }),
            Ok(Check::Unknown) => {
                let Some(trust) = &self.trust else { return Err(SshError::HostKeyRejected) };
                let query = HostKeyQuery {
                    host: name,
                    key_type: key.algorithm().as_str().to_string(),
                    fingerprint: fingerprint(&key),
                    cancel: self.cancel.clone(),
                };
                self.clock.pause();
                let ok = trust(query).await;
                self.clock.resume();
                if !ok {
                    return Err(SshError::HostKeyRejected);
                }
                self.store
                    .add(&self.host, self.port, &key)
                    .map_err(|e| SshError::Permanent(msg!("err.ssh.knownHostsWriteFailed", "写入 known_hosts 失败").with_cause(e)))?;
                Ok(true)
            }
        }
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<client::Msg>,
        _connected_address: &str,
        _connected_port: u32,
        originator_address: &str,
        originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), SshError> {
        if self.forwarded_tx.is_closed() {
            reply.reject(ChannelOpenFailure::AdministrativelyProhibited).await;
            return Ok(());
        }
        reply.accept().await;
        let origin = join_host_port(originator_address, originator_port);
        let _ = self.forwarded_tx.send(Forwarded { channel, origin });
        Ok(())
    }

    async fn disconnected(&mut self, reason: DisconnectReason<SshError>) -> Result<(), SshError> {
        let msg = match &reason {
            DisconnectReason::ReceivedDisconnect(info) if info.message.is_empty() => {
                msg!("err.ssh.serverClosed", "服务器关闭了连接")
            }
            DisconnectReason::ReceivedDisconnect(info) => msg!("err.ssh.serverClosed", "服务器关闭了连接").with_cause(&info.message),
            DisconnectReason::Error(e) => e.msg(),
        };
        if let Some(tx) = self.disconnect_tx.take() {
            let _ = tx.send(msg);
        }
        match reason {
            DisconnectReason::ReceivedDisconnect(_) => Ok(()),
            DisconnectReason::Error(e) => Err(e),
        }
    }
}

/// 包一层 TCP 连接：kill 取消后读写立即报错，SSH 会话随之结束。
/// 用来在握手卡住、用户停止隧道时可靠地关闭连接（russh 的会话跑在独立任务里）。
struct Killable {
    inner: TcpStream,
    killed: Pin<Box<WaitForCancellationFutureOwned>>,
}

impl Killable {
    fn check(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        match self.killed.as_mut().poll(cx) {
            Poll::Ready(()) => Err(io::Error::new(io::ErrorKind::ConnectionAborted, "连接已关闭")),
            Poll::Pending => Ok(()),
        }
    }
}

impl AsyncRead for Killable {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        self.check(cx)?;
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for Killable {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.check(cx)?;
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

pub struct SshClient {
    handle: Handle<ClientHandler>,
    kill: CancellationToken,
}

/// 一次成功的连接：客户端句柄、断开通知、远程转发进来的连接。
pub struct Connection {
    pub client: Arc<SshClient>,
    pub disconnected: oneshot::Receiver<Msg>,
    pub forwarded: mpsc::UnboundedReceiver<Forwarded>,
}

impl SshClient {
    /// 通过 direct-tcpip 让服务器去连接目标。
    pub async fn open_direct(&self, host: &str, port: u16, origin: Option<SocketAddr>) -> Result<Channel<client::Msg>, DialError> {
        let (oh, op) = origin.map(|a| (a.ip().to_string(), a.port() as u32)).unwrap_or(("127.0.0.1".into(), 0));
        self.handle
            .channel_open_direct_tcpip(host, port as u32, oh, op)
            .await
            .map_err(|e| match e {
                russh::Error::ChannelOpenFailure(ChannelOpenFailure::AdministrativelyProhibited) => {
                    DialError::Prohibited(format!("{host}:{port}"))
                }
                russh::Error::ChannelOpenFailure(f) => DialError::Unreachable(f.description().to_string()),
                russh::Error::SendError | russh::Error::Disconnect | russh::Error::RecvError => DialError::NotConnected,
                other => DialError::Other(describe(&other).text().to_string()),
            })
    }

    /// 请求服务器监听端口（远程转发）。
    pub async fn tcpip_forward(&self, host: &str, port: u16) -> Result<u32, SshError> {
        Ok(self.handle.tcpip_forward(host, port as u32).await?)
    }

    pub fn close(&self) {
        self.kill.cancel();
    }

    pub fn is_closed(&self) -> bool {
        self.kill.is_cancelled() || self.handle.is_closed()
    }
}

impl Drop for SshClient {
    fn drop(&mut self) {
        self.kill.cancel();
    }
}

/// 按隧道配置建立 SSH 连接并完成认证；cancel 取消会中断 TCP 连接和握手。
pub async fn connect(t: &Tunnel, opts: &DialOptions, cancel: &CancellationToken) -> Result<Connection, SshError> {
    let method = auth::prepare(t).await?;
    let addr = t.ssh_addr();
    let port = t.ssh_port as u16;

    let tcp = tokio::select! {
        _ = cancel.cancelled() => return Err(SshError::Cancelled),
        r = tokio::time::timeout(opts.timeout, TcpStream::connect((t.ssh_host.as_str(), port))) => match r {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(SshError::Other(msg!("err.ssh.connectFailed", "连接 {addr} 失败", addr).with_cause(e))),
            Err(_) => return Err(SshError::Other(msg!("err.ssh.connectTimeout", "连接 {addr} 超时", addr))),
        },
    };
    let _ = tcp.set_nodelay(true);

    let kill = cancel.child_token();
    let guard = kill.clone().drop_guard();
    let clock = Arc::new(PauseClock::default());
    let (disconnect_tx, disconnected) = oneshot::channel();
    let (forwarded_tx, forwarded) = mpsc::unbounded_channel();
    let handler = ClientHandler {
        host: t.ssh_host.clone(),
        port,
        store: opts.host_keys.clone(),
        trust: opts.trust.clone(),
        cancel: kill.clone(),
        clock: clock.clone(),
        disconnect_tx: Some(disconnect_tx),
        forwarded_tx,
    };

    let mut config = client::Config {
        keepalive_interval: opts.keepalive,
        keepalive_max: opts.keepalive_max,
        nodelay: true,
        ..Default::default()
    };
    let algos = opts.host_keys.known_algorithms(&t.ssh_host, port);
    if !algos.is_empty() {
        config.preferred.key = Cow::Owned(algos);
    }

    let stream = Killable { inner: tcp, killed: Box::pin(kill.clone().cancelled_owned()) };
    let start = Instant::now();
    let handshake = async {
        let mut handle = client::connect_stream(Arc::new(config), stream, handler).await?;
        auth::authenticate(&mut handle, &t.ssh_user, method).await?;
        Ok::<_, SshError>(handle)
    };
    let handle = tokio::select! {
        r = handshake => r?,
        _ = cancel.cancelled() => return Err(SshError::Cancelled),
        _ = clock.expired(start, opts.timeout) => {
            return Err(SshError::Other(msg!("err.ssh.handshakeTimeout", "与 {addr} 的 SSH 握手超时", addr)));
        }
    };
    guard.disarm();
    Ok(Connection { client: Arc::new(SshClient { handle, kill }), disconnected, forwarded })
}
