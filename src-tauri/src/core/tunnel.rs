use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Local;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::core::forward::{self, BoxStream, DialError, DialFn, Server};
use crate::core::ssh::known_hosts::HostKeyStore;
use crate::core::ssh::{self, DialOptions, Forwarded, SshClient, SshError, TrustFn};
use crate::core::stats::Collector;
use crate::i18n::Msg;
use crate::model::{Tunnel, TunnelState, TunnelStatus, TunnelType};

pub type StatusFn = Arc<dyn Fn(TunnelStatus) + Send + Sync>;

#[derive(Clone)]
pub struct Options {
    pub host_keys: Arc<HostKeyStore>,
    pub trust: Option<TrustFn>,
    /// 状态变化时调用（在隧道内部任务中，不要阻塞）。
    pub on_status: Option<StatusFn>,
    pub dial_timeout: Duration,
    /// 本地连接进来时 SSH 还没连上，最多等这么久。
    pub wait_connected: Duration,
    /// 非空时覆盖隧道配置中按秒设置的保活间隔（测试用）。
    pub keepalive_interval: Option<Duration>,
    pub keepalive_missed: usize,
    pub min_backoff: Duration,
    pub max_backoff: Duration,
}

impl Options {
    pub fn new(host_keys: Arc<HostKeyStore>) -> Self {
        Options {
            host_keys,
            trust: None,
            on_status: None,
            dial_timeout: Duration::from_secs(15),
            wait_connected: Duration::from_secs(10),
            keepalive_interval: None,
            keepalive_missed: 3,
            min_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
        }
    }
}

struct State {
    status: TunnelStatus,
    started: bool,
    local: Option<Arc<Server>>,
    cancel: Option<CancellationToken>,
    task: Option<JoinHandle<()>>,
}

/// 维护一条 SSH 连接和对应的端口转发。
/// 本地、动态转发的本机端口在断线重连期间保持监听；远程转发在每次连上后重新向服务器申请监听。
pub struct Runner {
    def: Tunnel,
    opts: Options,
    stats: Arc<Collector>,
    state: Mutex<State>,
    client: watch::Sender<Option<Arc<SshClient>>>,
    /// 串行化 start/stop。
    op: tokio::sync::Mutex<()>,
}

impl Runner {
    pub fn new(mut def: Tunnel, opts: Options) -> Arc<Self> {
        def.normalize();
        let status = TunnelStatus::stopped(&def);
        Arc::new(Runner {
            def,
            opts,
            stats: Collector::new(),
            state: Mutex::new(State { status, started: false, local: None, cancel: None, task: None }),
            client: watch::channel(None).0,
            op: tokio::sync::Mutex::new(()),
        })
    }

    pub fn def(&self) -> &Tunnel {
        &self.def
    }

    pub fn stats(&self) -> &Arc<Collector> {
        &self.stats
    }

    pub fn status(&self) -> TunnelStatus {
        self.state.lock().unwrap().status.clone()
    }

    /// 实际监听地址（本机端口配置为 0 时可拿到系统分配的端口）。
    pub fn listen_addr(&self) -> String {
        self.status().listen_addr
    }

    pub fn running(&self) -> bool {
        self.state.lock().unwrap().started
    }

    /// 本地、动态转发会先绑定本机端口（被占用时立即返回错误），然后在后台连接 SSH。
    pub async fn start(self: &Arc<Self>) -> Result<(), Msg> {
        self.def.validate()?;
        let _op = self.op.lock().await;
        if self.running() {
            return Err(msg!("err.tunnel.alreadyRunning", "隧道已在运行"));
        }

        let mut local = None;
        let mut listen_addr = self.def.listen_addr();
        if self.def.kind != TunnelType::Remote {
            let ln = TcpListener::bind(listen_addr.as_str()).await.map_err(|e| listen_error(&listen_addr, &e))?;
            if let Ok(a) = ln.local_addr() {
                listen_addr = a.to_string();
            }
            let srv = self.new_local_server();
            srv.serve(ln);
            info!(tunnel = %self.def.name, kind = self.def.kind.as_str(), addr = %listen_addr, "本地端口已监听");
            local = Some(srv);
        }

        let cancel = CancellationToken::new();
        {
            let mut st = self.state.lock().unwrap();
            st.status.listen_addr = listen_addr;
            st.local = local;
            st.cancel = Some(cancel.clone());
            st.started = true;
        }
        let me = self.clone();
        let task = tokio::spawn(async move { me.run(cancel).await });
        self.state.lock().unwrap().task = Some(task);
        Ok(())
    }

    /// 断开 SSH、关闭监听和所有转发连接，可重复调用。
    pub async fn stop(&self) {
        let _op = self.op.lock().await;
        let (cancel, task, local) = {
            let mut st = self.state.lock().unwrap();
            if !st.started {
                return;
            }
            (st.cancel.take(), st.task.take(), st.local.take())
        };
        if let Some(c) = cancel {
            c.cancel();
        }
        if let Some(srv) = local {
            srv.close().await;
        }
        if let Some(t) = task {
            let _ = t.await;
        }
        self.state.lock().unwrap().started = false;
        self.set_state(TunnelState::Stopped, None, 0);
        info!(tunnel = %self.def.name, "隧道已停止");
    }

    fn new_local_server(&self) -> Arc<Server> {
        let mut opts = forward::Options::new(self.ssh_dialer(), self.stats.clone());
        opts.idle_timeout = idle(self.def.idle_timeout_seconds);
        opts.max_conns = self.def.max_conns.max(0) as usize;
        opts.name = self.def.name.clone();
        if self.def.kind == TunnelType::Local {
            opts.target = Some((self.def.target_host.clone(), self.def.target_port as u16));
            opts.proto = "local";
        } else {
            opts.enable_http = self.def.enable_http;
            if !self.def.proxy_user.is_empty() {
                opts.auth = Some(forward::Credentials {
                    user: self.def.proxy_user.clone(),
                    pass: self.def.proxy_pass.clone(),
                });
            }
        }
        Server::new(opts)
    }

    /// 供本地监听使用：SSH 未连接时最多等待 wait_connected，然后通过 direct-tcpip 让服务器去连目标。
    fn ssh_dialer(&self) -> DialFn {
        let rx = self.client.subscribe();
        let wait = self.opts.wait_connected;
        Arc::new(move |host, port, origin| {
            let mut rx = rx.clone();
            Box::pin(async move {
                let client = match tokio::time::timeout(wait, rx.wait_for(|c| c.is_some())).await {
                    Ok(Ok(c)) => c.clone(),
                    _ => None,
                };
                let client = client.ok_or(DialError::NotConnected)?;
                let ch = client.open_direct(&host, port, origin).await?;
                Ok(Box::new(ch.into_stream()) as BoxStream)
            })
        })
    }

    async fn run(self: Arc<Self>, cancel: CancellationToken) {
        let mut attempt = 0u32;
        loop {
            let state = if attempt > 0 { TunnelState::Reconnecting } else { TunnelState::Connecting };
            self.set_state(state, None, attempt);

            let err = self.session(&cancel, &mut attempt).await;
            self.client.send_replace(None);
            if cancel.is_cancelled() {
                return;
            }
            if err.is_permanent() {
                error!(tunnel = %self.def.name, err = %err, "连接失败，停止重试");
                self.fail(&err, attempt).await;
                return;
            }
            if !self.def.auto_reconnect {
                warn!(tunnel = %self.def.name, err = %err, "连接断开，未开启自动重连");
                self.fail(&err, attempt).await;
                return;
            }
            attempt += 1;
            let wait = self.backoff(attempt);
            warn!(tunnel = %self.def.name, err = %err, attempt, wait_ms = wait.as_millis() as u64, "连接断开或失败，稍后重试");
            self.set_state(TunnelState::Reconnecting, Some(err.msg()), attempt);
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }

    /// 一次“连接 → 开始转发 → 等待断开”，返回断开原因。连接成功会把 attempt 清零。
    async fn session(&self, cancel: &CancellationToken, attempt: &mut u32) -> SshError {
        let keepalive = self.opts.keepalive_interval.or_else(|| idle(self.def.keepalive_seconds));
        let dial = DialOptions {
            host_keys: self.opts.host_keys.clone(),
            trust: self.opts.trust.clone(),
            timeout: self.opts.dial_timeout,
            keepalive,
            keepalive_max: self.opts.keepalive_missed,
        };
        let ssh::Connection { client, mut disconnected, mut forwarded } = match ssh::connect(&self.def, &dial, cancel).await {
            Ok(c) => c,
            Err(e) => return e,
        };

        let mut remote = None;
        if self.def.kind == TunnelType::Remote {
            if let Err(e) = client.tcpip_forward(&self.def.listen_host, self.def.listen_port as u16).await {
                client.close();
                let addr = self.def.listen_addr();
                return SshError::Other(
                    msg!("err.tunnel.remoteListenFailed", "在服务器上监听 {addr} 失败（端口被占用，或服务器未允许该地址）", addr)
                        .with_cause(e),
                );
            }
            let mut opts = forward::Options::new(forward::tcp_dialer(), self.stats.clone());
            opts.target = Some((self.def.target_host.clone(), self.def.target_port as u16));
            opts.proto = "remote";
            opts.idle_timeout = idle(self.def.idle_timeout_seconds);
            opts.max_conns = self.def.max_conns.max(0) as usize;
            opts.name = self.def.name.clone();
            remote = Some(Server::new(opts));
            info!(tunnel = %self.def.name, addr = %self.def.listen_addr(), target = %self.def.target_addr(), "服务器端口已监听");
        }

        *attempt = 0;
        self.client.send_replace(Some(client.clone()));
        self.set_state(TunnelState::Connected, None, 0);
        info!(tunnel = %self.def.name, server = %self.def.ssh_addr(), "SSH 已连接");

        let accept_forwarded = async {
            while let Some(Forwarded { channel, origin }) = forwarded.recv().await {
                if let Some(srv) = &remote {
                    srv.handle(Box::new(channel.into_stream()), origin, None);
                }
            }
            std::future::pending::<()>().await
        };
        let err = tokio::select! {
            _ = cancel.cancelled() => SshError::Cancelled,
            r = &mut disconnected => SshError::Other(r.unwrap_or_else(|_| msg!("err.ssh.connectionLost", "连接断开"))),
            _ = accept_forwarded => unreachable!(),
        };
        self.client.send_replace(None);
        // 先关 SSH 连接再关远程转发的连接，避免向已经卡死的连接发送请求而阻塞。
        client.close();
        if let Some(srv) = remote {
            srv.close().await;
        }
        err
    }

    async fn fail(&self, err: &SshError, attempt: u32) {
        let local = self.state.lock().unwrap().local.take();
        if let Some(srv) = local {
            srv.close().await;
        }
        self.set_state(TunnelState::Failed, Some(err.msg()), attempt);
    }

    fn backoff(&self, attempt: u32) -> Duration {
        let mut d = self.opts.min_backoff;
        for _ in 1..attempt {
            if d >= self.opts.max_backoff {
                break;
            }
            d *= 2;
        }
        let d = d.min(self.opts.max_backoff);
        // 加 ±20% 抖动，避免多条隧道同时重连。
        d.mul_f64(0.8 + 0.4 * jitter())
    }

    fn set_state(&self, state: TunnelState, err: Option<Msg>, attempt: u32) {
        let st = {
            let mut s = self.state.lock().unwrap();
            s.status.state = state;
            s.status.since = Local::now();
            s.status.attempt = attempt;
            s.status.error = err;
            s.status.clone()
        };
        if let Some(f) = &self.opts.on_status {
            f(st);
        }
    }
}

fn idle(seconds: i32) -> Option<Duration> {
    (seconds > 0).then(|| Duration::from_secs(seconds as u64))
}

/// [0, 1) 之间的伪随机数，只用于重连抖动。
fn jitter() -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let mut x = nanos as u64 ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    (x % 10_000) as f64 / 10_000.0
}

/// 把常见的监听失败原因翻译成用户能直接处理的提示。
fn listen_error(addr: &str, e: &std::io::Error) -> Msg {
    let m = match e.kind() {
        std::io::ErrorKind::AddrInUse => {
            msg!("err.listen.addrInUse", "监听 {addr} 失败：端口已被其他程序占用，请换一个端口", addr)
        }
        std::io::ErrorKind::PermissionDenied if cfg!(windows) => msg!(
            "err.listen.reserved",
            "监听 {addr} 失败：端口被系统保留（常见于 Hyper-V / WSL），请换一个端口；\
             可用 netsh int ipv4 show excludedportrange protocol=tcp 查看保留范围",
            addr
        ),
        std::io::ErrorKind::PermissionDenied => {
            msg!("err.listen.permissionDenied", "监听 {addr} 失败：没有权限，1024 以下端口需要管理员权限，请换一个端口", addr)
        }
        std::io::ErrorKind::AddrNotAvailable => msg!("err.listen.addrNotAvailable", "监听 {addr} 失败：本机没有这个地址", addr),
        _ => msg!("err.listen.failed", "监听 {addr} 失败", addr),
    };
    m.with_cause(e)
}
