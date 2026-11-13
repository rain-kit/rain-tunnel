//! 接收连接并转发：端口转发模式直接连固定目标；代理模式在一个端口上同时提供
//! SOCKS5 和（可选的）HTTP 代理，按首字节区分协议。

pub mod http;
pub mod relay;
pub mod socks5;

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use subtle::ConstantTimeEq;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, info, warn};

use crate::core::stats::{Collector, Conn};
use crate::util::join_host_port;
use relay::{Activity, RelayError, is_closed_err, pipe, relay};

pub trait Stream: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> Stream for T {}
pub type BoxStream = Box<dyn Stream>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum DialError {
    #[error("SSH 未连接")]
    NotConnected,
    #[error("服务器禁止转发到该地址: {0}")]
    Prohibited(String),
    #[error("服务器连接目标失败: {0}")]
    Unreachable(String),
    #[error("连接目标失败: {0}")]
    Refused(String),
    #[error("连接目标超时")]
    Timeout,
    #[error("{0}")]
    Other(String),
}

impl DialError {
    fn socks_code(&self) -> u8 {
        match self {
            DialError::Prohibited(_) => socks5::REP_NOT_ALLOWED,
            DialError::Unreachable(_) | DialError::Timeout => socks5::REP_HOST_UNREACHABLE,
            DialError::Refused(_) => socks5::REP_CONNECTION_REFUSED,
            DialError::NotConnected | DialError::Other(_) => socks5::REP_GENERAL_FAILURE,
        }
    }
}

pub type DialFuture = Pin<Box<dyn Future<Output = Result<BoxStream, DialError>> + Send>>;
/// 建立到目标 (host, port) 的连接；第三个参数是发起连接的客户端地址（用于 direct-tcpip 的来源字段）。
pub type DialFn = Arc<dyn Fn(String, u16, Option<SocketAddr>) -> DialFuture + Send + Sync>;

/// 直接用本机 TCP 连接目标（远程转发时由本机去连接目标）。
pub fn tcp_dialer() -> DialFn {
    Arc::new(|host, port, _| {
        Box::pin(async move {
            match tokio::net::TcpStream::connect((host.as_str(), port)).await {
                Ok(s) => {
                    let _ = s.set_nodelay(true);
                    Ok(Box::new(s) as BoxStream)
                }
                Err(e) => Err(DialError::Refused(e.to_string())),
            }
        })
    })
}

#[derive(Clone)]
pub struct Credentials {
    pub user: String,
    pub pass: String,
}

impl Credentials {
    pub fn matches(&self, user: &[u8], pass: &[u8]) -> bool {
        let u = user.ct_eq(self.user.as_bytes());
        let p = pass.ct_eq(self.pass.as_bytes());
        (u & p).into()
    }
}

pub struct Options {
    pub dial: DialFn,
    /// 非空时为端口转发模式：所有连接直接转到该地址，不解析代理协议。
    pub target: Option<(String, u16)>,
    /// 写入连接统计的协议名，端口转发模式下使用，如 "local"、"remote"。
    pub proto: &'static str,
    /// 代理模式下的认证，None 表示不需要。
    pub auth: Option<Credentials>,
    pub enable_http: bool,
    pub handshake_timeout: Duration,
    pub dial_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    /// 0 表示不限制。
    pub max_conns: usize,
    pub stats: Arc<Collector>,
    /// 日志里标识所属隧道。
    pub name: String,
}

impl Options {
    pub fn new(dial: DialFn, stats: Arc<Collector>) -> Self {
        Options {
            dial,
            target: None,
            proto: "",
            auth: None,
            enable_http: false,
            handshake_timeout: Duration::from_secs(10),
            dial_timeout: Duration::from_secs(20),
            idle_timeout: None,
            max_conns: 0,
            stats,
            name: String::new(),
        }
    }
}

pub struct Server {
    opts: Options,
    cancel: CancellationToken,
    tracker: TaskTracker,
    sem: Option<Arc<Semaphore>>,
}

impl Server {
    pub fn new(opts: Options) -> Arc<Self> {
        let sem = (opts.max_conns > 0).then(|| Arc::new(Semaphore::new(opts.max_conns)));
        Arc::new(Server { opts, cancel: CancellationToken::new(), tracker: TaskTracker::new(), sem })
    }

    /// 在后台接收 listener 上的连接，直到 close。
    pub fn serve(self: &Arc<Self>, listener: TcpListener) {
        let srv = self.clone();
        self.tracker.spawn(async move {
            let mut backoff = Duration::ZERO;
            loop {
                let accepted = tokio::select! {
                    _ = srv.cancel.cancelled() => return,
                    r = listener.accept() => r,
                };
                match accepted {
                    Ok((stream, addr)) => {
                        backoff = Duration::ZERO;
                        let _ = stream.set_nodelay(true);
                        srv.handle(Box::new(stream), addr.to_string(), Some(addr));
                    }
                    Err(e) => {
                        // 句柄耗尽等临时错误：退避后继续，避免空转。
                        backoff = (backoff * 2).clamp(Duration::from_millis(5), Duration::from_secs(1));
                        warn!(tunnel = %srv.opts.name, err = %e, "accept 失败");
                        tokio::select! {
                            _ = srv.cancel.cancelled() => return,
                            _ = tokio::time::sleep(backoff) => {}
                        }
                    }
                }
            }
        });
    }

    /// 处理一条已建立的连接（本机监听接入的 TCP，或远程转发时服务器开过来的通道）。
    pub fn handle(self: &Arc<Self>, stream: BoxStream, client: String, client_addr: Option<SocketAddr>) {
        if self.cancel.is_cancelled() {
            return;
        }
        let permit = match &self.sem {
            Some(sem) => match sem.clone().try_acquire_owned() {
                Ok(p) => Some(p),
                Err(_) => {
                    warn!(tunnel = %self.opts.name, max = self.opts.max_conns, client = %client, "连接数已达上限，拒绝新连接");
                    return;
                }
            },
            None => None,
        };
        let srv = self.clone();
        let token = self.cancel.child_token();
        self.tracker.spawn(async move {
            let _permit = permit;
            let kill = token.clone();
            tokio::select! {
                _ = token.cancelled() => {}
                _ = srv.serve_conn(stream, client, client_addr, kill) => {}
            }
        });
    }

    /// 关闭监听和所有活动连接，并等待处理任务退出。
    pub async fn close(&self) {
        self.cancel.cancel();
        self.tracker.close();
        self.tracker.wait().await;
    }

    async fn serve_conn(&self, stream: BoxStream, client: String, addr: Option<SocketAddr>, kill: CancellationToken) {
        if let Some((host, port)) = self.opts.target.clone() {
            return self.serve_fixed(stream, client, addr, host, port, kill).await;
        }
        let mut c = BufReader::new(stream);
        let first = match tokio::time::timeout(self.opts.handshake_timeout, c.fill_buf()).await {
            Ok(Ok([b, ..])) => *b,
            _ => return,
        };
        if first == socks5::VERSION {
            self.serve_socks5(c, client, addr, kill).await;
        } else if self.opts.enable_http && http::is_method_start(first) {
            self.serve_http(c, client, addr, kill).await;
        } else {
            debug!(tunnel = %self.opts.name, client = %client, byte = first, "未知协议，关闭连接");
        }
    }

    fn open_conn(&self, proto: &'static str, client: &str, target: &str, kill: CancellationToken) -> Conn {
        self.opts.stats.open(proto, client, target, move || kill.cancel())
    }

    async fn dial(&self, host: &str, port: u16, addr: Option<SocketAddr>) -> Result<BoxStream, DialError> {
        match tokio::time::timeout(self.opts.dial_timeout, (self.opts.dial)(host.to_string(), port, addr)).await {
            Ok(r) => r,
            Err(_) => Err(DialError::Timeout),
        }
    }

    async fn serve_fixed(
        &self,
        stream: BoxStream,
        client: String,
        addr: Option<SocketAddr>,
        host: String,
        port: u16,
        kill: CancellationToken,
    ) {
        let target = join_host_port(&host, port);
        let conn = self.open_conn(self.opts.proto, &client, &target, kill);
        let dst = match self.dial(&host, port, addr).await {
            Ok(d) => d,
            Err(e) => {
                info!(tunnel = %self.opts.name, proto = self.opts.proto, target = %target, err = %e, "连接目标失败");
                conn.close(Some(e.to_string()));
                return;
            }
        };
        let r = relay(stream, dst, &conn, self.opts.idle_timeout).await;
        conn.close(r.err().map(|e| e.to_string()));
    }

    async fn serve_socks5(&self, mut c: BufReader<BoxStream>, client: String, addr: Option<SocketAddr>, kill: CancellationToken) {
        let handshake = async {
            socks5::negotiate(&mut c, self.opts.auth.as_ref()).await?;
            socks5::read_request(&mut c).await
        };
        let (host, port) = match tokio::time::timeout(self.opts.handshake_timeout, handshake).await {
            Ok(Ok(t)) => t,
            Ok(Err(socks5::Socks5Error::Reply { code, msg })) => {
                let _ = socks5::write_reply(&mut c, code).await;
                debug!(tunnel = %self.opts.name, client = %client, err = %msg, "SOCKS5 请求无效");
                return;
            }
            Ok(Err(e)) => {
                debug!(tunnel = %self.opts.name, client = %client, err = %e, "SOCKS5 握手失败");
                return;
            }
            Err(_) => return,
        };
        let target = join_host_port(&host, port);
        let conn = self.open_conn("socks5", &client, &target, kill);
        let dst = match self.dial(&host, port, addr).await {
            Ok(d) => d,
            Err(e) => {
                let _ = socks5::write_reply(&mut c, e.socks_code()).await;
                info!(tunnel = %self.opts.name, proto = "socks5", target = %target, err = %e, "连接目标失败");
                conn.close(Some(e.to_string()));
                return;
            }
        };
        if let Err(e) = socks5::write_reply(&mut c, socks5::REP_SUCCEEDED).await {
            conn.close((!is_closed_err(&e)).then(|| e.to_string()));
            return;
        }
        debug!(tunnel = %self.opts.name, target = %target, "开始转发");
        let r = relay(c, dst, &conn, self.opts.idle_timeout).await;
        conn.close(r.err().map(|e| e.to_string()));
    }

    async fn serve_http(&self, mut c: BufReader<BoxStream>, client: String, addr: Option<SocketAddr>, kill: CancellationToken) {
        let head = match tokio::time::timeout(self.opts.handshake_timeout, http::read_head(&mut c)).await {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => {
                debug!(tunnel = %self.opts.name, client = %client, err = %e, "HTTP 请求解析失败");
                return;
            }
            Err(_) => return,
        };
        if let Some(auth) = &self.opts.auth
            && !http::check_auth(&head, auth)
        {
            let _ = c.write_all(http::AUTH_REQUIRED.as_bytes()).await;
            return;
        }

        if head.method.eq_ignore_ascii_case("CONNECT") {
            let Some((host, port)) = http::split_host_port(&head.uri, 443) else {
                let _ = http::write_error(&mut c, 400, "Bad Request", "CONNECT 目标地址无效").await;
                return;
            };
            let target = join_host_port(&host, port);
            let conn = self.open_conn("http-connect", &client, &target, kill);
            let dst = match self.dial(&host, port, addr).await {
                Ok(d) => d,
                Err(e) => {
                    let _ = http::write_error(&mut c, 502, "Bad Gateway", &e.to_string()).await;
                    info!(tunnel = %self.opts.name, proto = "http", target = %target, err = %e, "连接目标失败");
                    conn.close(Some(e.to_string()));
                    return;
                }
            };
            if let Err(e) = c.write_all(http::CONNECT_OK.as_bytes()).await {
                conn.close((!is_closed_err(&e)).then(|| e.to_string()));
                return;
            }
            let r = relay(c, dst, &conn, self.opts.idle_timeout).await;
            conn.close(r.err().map(|e| e.to_string()));
            return;
        }

        // 普通 HTTP 代理请求：每个连接只转发一个请求，并要求上游在响应后关闭连接，
        // 这样不用解析响应就能确定边界；绝大多数 HTTPS 流量走 CONNECT，不受影响。
        let Some((host, port, path, authority)) = http::split_absolute_uri(&head.uri) else {
            let _ = http::write_error(&mut c, 400, "Bad Request", "只支持绝对地址的 http:// 请求").await;
            return;
        };
        let target = join_host_port(&host, port);
        let conn = self.open_conn("http", &client, &target, kill);
        let mut dst = match self.dial(&host, port, addr).await {
            Ok(d) => d,
            Err(e) => {
                let _ = http::write_error(&mut c, 502, "Bad Gateway", &e.to_string()).await;
                info!(tunnel = %self.opts.name, proto = "http", target = %target, err = %e, "连接目标失败");
                conn.close(Some(e.to_string()));
                return;
            }
        };
        let req = http::rewrite_head(&head, &path, &authority);
        if let Err(e) = dst.write_all(&req).await {
            let _ = http::write_error(&mut c, 502, "Bad Gateway", &e.to_string()).await;
            conn.close(Some(e.to_string()));
            return;
        }
        conn.add_up(req.len() as u64);

        let (mut cr, mut cw) = tokio::io::split(c);
        let (mut dr, mut dw) = tokio::io::split(dst);
        let act = Activity::new();
        // 请求体继续透传；上游响应结束（关闭连接）即完成。
        let up = async {
            let _ = pipe(&mut cr, &mut dw, |n| conn.add_up(n), &act).await;
            std::future::pending::<()>().await
        };
        let down = pipe(&mut dr, &mut cw, |n| conn.add_down(n), &act);
        let r: Result<(), RelayError> = tokio::select! {
            _ = up => Ok(()),
            r = down => r.map_err(RelayError::from),
            _ = act.idle_opt(self.opts.idle_timeout) => Err(RelayError::Idle),
        };
        let err = match r {
            Err(RelayError::Io(e)) if is_closed_err(&e) => None,
            r => r.err().map(|e| e.to_string()),
        };
        conn.close(err);
    }
}
