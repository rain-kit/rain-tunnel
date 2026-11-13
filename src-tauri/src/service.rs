use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use chrono::Local;
use tokio::sync::oneshot;
use tracing::{error, info, warn};

use crate::core::ssh::known_hosts::HostKeyStore;
use crate::core::ssh::{HostKeyQuery, TrustFn};
use crate::core::tunnel::{self, Runner};
use crate::error::{self, ApiResult};
use crate::events::{self, EventSink};
use crate::i18n::Msg;
use crate::model::{ConnectionView, HostKeyPrompt, SaveTunnelReq, Tunnel, TunnelState, TunnelStatus, TunnelView};
use crate::secrets::{self, Secrets};
use crate::store::{StoreError, TunnelStore};
use crate::util::new_id;

const HOST_KEY_PROMPT_TIMEOUT: Duration = Duration::from_secs(120);

struct Prompt {
    info: HostKeyPrompt,
    answer: Option<oneshot::Sender<bool>>,
}

/// 可调参数，测试中用来缩短超时和重连间隔。
#[derive(Clone)]
pub struct Tuning {
    pub dial_timeout: Duration,
    pub wait_connected: Duration,
    pub keepalive_interval: Option<Duration>,
    pub min_backoff: Duration,
    pub max_backoff: Duration,
    pub prompt_timeout: Duration,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            dial_timeout: Duration::from_secs(15),
            wait_connected: Duration::from_secs(10),
            keepalive_interval: None,
            min_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            prompt_timeout: HOST_KEY_PROMPT_TIMEOUT,
        }
    }
}

pub struct TunnelService {
    store: TunnelStore,
    secrets: Secrets,
    host_keys: Arc<HostKeyStore>,
    events: Arc<dyn EventSink>,
    tuning: Tuning,
    /// 已启动（含连接失败）的隧道；停止后移除。
    running: Mutex<HashMap<String, Arc<Runner>>>,
    prompts: Mutex<HashMap<String, Prompt>>,
    on_change: Mutex<Vec<Arc<dyn Fn() + Send + Sync>>>,
    /// 串行化启动、停止，避免同一条隧道被并发启动两次。
    op: tokio::sync::Mutex<()>,
    me: Weak<TunnelService>,
}

/// 读写 tunnels.json 失败时，IO 错误归为 fallback。
fn store_err_or(e: StoreError, fallback: error::Code) -> error::ApiError {
    match e {
        StoreError::NotFound => error::TUNNEL_NOT_FOUND.err(),
        StoreError::Json(e) => error::TUNNEL_FILE_INVALID.cause(e),
        StoreError::Io(e) => fallback.cause(e),
    }
}

fn store_err(e: StoreError) -> error::ApiError {
    store_err_or(e, error::SERVICE_ERROR)
}

impl TunnelService {
    pub fn new(
        store: TunnelStore,
        secrets: Secrets,
        host_keys: Arc<HostKeyStore>,
        events: Arc<dyn EventSink>,
        tuning: Tuning,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| TunnelService {
            store,
            secrets,
            host_keys,
            events,
            tuning,
            running: Mutex::new(HashMap::new()),
            prompts: Mutex::new(HashMap::new()),
            on_change: Mutex::new(Vec::new()),
            op: tokio::sync::Mutex::new(()),
            me: me.clone(),
        })
    }

    /// 注册隧道状态或列表变化的回调（用于刷新托盘菜单）。
    pub fn on_change(&self, f: impl Fn() + Send + Sync + 'static) {
        self.on_change.lock().unwrap().push(Arc::new(f));
    }

    fn notify_change(&self) {
        let fns = self.on_change.lock().unwrap().clone();
        for f in fns {
            f();
        }
    }

    fn changed(&self) {
        self.events.emit(events::TUNNEL_CHANGED, serde_json::Value::Null);
        self.notify_change();
    }

    fn get_running(&self, id: &str) -> Option<Arc<Runner>> {
        self.running.lock().unwrap().get(id).cloned()
    }

    pub fn snapshot_running(&self) -> HashMap<String, Arc<Runner>> {
        self.running.lock().unwrap().clone()
    }

    pub fn list(&self) -> ApiResult<Vec<TunnelView>> {
        let list = self.store.list().map_err(|e| {
            error!(err = %e, "读取隧道列表失败");
            store_err_or(e, error::TUNNEL_LIST_FAILED)
        })?;
        Ok(list.into_iter().map(|t| self.view(t)).collect())
    }

    /// 隧道名称和状态，不读凭据库，供托盘菜单频繁刷新使用。
    pub fn list_status(&self) -> Vec<(Tunnel, TunnelStatus)> {
        let list = self.store.list().unwrap_or_default();
        list.into_iter()
            .map(|t| {
                let st = self.get_running(&t.id).map(|r| r.status()).unwrap_or_else(|| TunnelStatus::stopped(&t));
                (t, st)
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> ApiResult<TunnelView> {
        self.store.get(id).map(|t| self.view(t)).map_err(store_err)
    }

    fn view(&self, mut t: Tunnel) -> TunnelView {
        let has_password = self.secrets.has(&t.id, secrets::PASSWORD);
        let has_key_passphrase = self.secrets.has(&t.id, secrets::KEY_PASSPHRASE);
        let has_proxy_pass = self.secrets.has(&t.id, secrets::PROXY_PASS);
        t.clear_secrets();
        let (status, totals) = match self.get_running(&t.id) {
            Some(r) => (r.status(), r.stats().totals()),
            None => (TunnelStatus::stopped(&t), Default::default()),
        };
        TunnelView {
            tunnel: t,
            has_password,
            has_key_passphrase,
            has_proxy_pass,
            status,
            bytes_up: totals.up,
            bytes_down: totals.down,
            active_conns: totals.active,
        }
    }

    /// 新建（ID 为空）或修改隧道。运行中的隧道修改后会自动重启以应用新配置。
    pub async fn save(&self, req: SaveTunnelReq) -> ApiResult<TunnelView> {
        let mut t = req.tunnel;
        let now = Local::now();
        let is_new = t.id.is_empty();
        if is_new {
            t.id = new_id();
            t.created_at = Some(now);
        } else {
            let old = self.store.get(&t.id).map_err(store_err)?;
            t.created_at = old.created_at;
        }
        t.updated_at = Some(now);
        t.normalize();

        // 校验时需要密码字段：留空表示沿用已保存的值。
        let mut check = t.clone();
        self.secrets.fill(&mut check).map_err(|e| error::SECRET_READ_FAILED.cause(e))?;
        let overlay = |dst: &mut String, v: &str, clear: bool| {
            if clear {
                dst.clear();
            } else if !v.is_empty() {
                *dst = v.to_string();
            }
        };
        overlay(&mut check.password, &t.password, req.clear_password);
        overlay(&mut check.key_passphrase, &t.key_passphrase, req.clear_key_passphrase);
        overlay(&mut check.proxy_pass, &t.proxy_pass, req.clear_proxy_pass);
        check.validate().map_err(|e| error::TUNNEL_INVALID.msg(e))?;

        let secrets = [
            (secrets::PASSWORD, &t.password, req.clear_password),
            (secrets::KEY_PASSPHRASE, &t.key_passphrase, req.clear_key_passphrase),
            (secrets::PROXY_PASS, &t.proxy_pass, req.clear_proxy_pass),
        ];
        for (kind, value, clear) in secrets {
            if value.is_empty() && !clear {
                continue;
            }
            self.secrets.set(&t.id, kind, value).map_err(|e| {
                error!(tunnel = %t.name, err = %e, "保存密码失败");
                error::SECRET_STORE_FAILED.cause(e)
            })?;
        }
        self.store.save_one(t.clone()).map_err(|e| store_err_or(e, error::TUNNEL_SAVE_FAILED))?;
        info!(tunnel = %t.name, new = is_new, "隧道已保存");

        if !is_new && self.get_running(&t.id).is_some() {
            self.stop(&t.id).await;
            if let Err(e) = self.start(&t.id).await {
                self.changed();
                return Err(e);
            }
        }
        self.changed();
        Ok(self.view(t))
    }

    /// 复制一条隧道（含密码），名称加“副本”，不自动启动。
    pub async fn duplicate(&self, id: &str) -> ApiResult<TunnelView> {
        let mut t = self.store.get(id).map_err(store_err)?;
        self.secrets.fill(&mut t).map_err(|e| error::SECRET_READ_FAILED.cause(e))?;
        t.id.clear();
        t.name.push_str(crate::i18n::pick([" 副本", " 副本", " Copy"]));
        t.auto_start = false;
        self.save(SaveTunnelReq { tunnel: t, ..Default::default() }).await
    }

    pub async fn delete(&self, id: &str) -> ApiResult<()> {
        self.stop(id).await;
        self.store.delete(id).map_err(store_err)?;
        if let Err(e) = self.secrets.delete_all(id) {
            warn!(id, err = %e, "删除隧道密码失败");
        }
        self.changed();
        Ok(())
    }

    pub fn reorder(&self, ids: &[String]) -> ApiResult<()> {
        self.store.reorder(ids).map_err(store_err)?;
        self.changed();
        Ok(())
    }

    pub async fn start(&self, id: &str) -> ApiResult<()> {
        let mut t = self.store.get(id).map_err(store_err)?;
        self.secrets.fill(&mut t).map_err(|e| error::SECRET_READ_FAILED.cause(e))?;

        let _op = self.op.lock().await;
        let old = {
            let mut running = self.running.lock().unwrap();
            match running.get(id) {
                // 失败的隧道允许直接再次启动。
                Some(r) if r.status().state != TunnelState::Failed => return Ok(()),
                _ => running.remove(id),
            }
        };
        if let Some(old) = old {
            old.stop().await;
        }

        let mut opts = tunnel::Options::new(self.host_keys.clone());
        opts.trust = Some(self.trust_fn(&t));
        let me = self.me.clone();
        opts.on_status = Some(Arc::new(move |st| {
            if let Some(svc) = me.upgrade() {
                svc.events.emit(events::TUNNEL_STATUS, serde_json::to_value(&st).unwrap_or_default());
                svc.notify_change();
            }
        }));
        opts.dial_timeout = self.tuning.dial_timeout;
        opts.wait_connected = self.tuning.wait_connected;
        opts.keepalive_interval = self.tuning.keepalive_interval;
        opts.min_backoff = self.tuning.min_backoff;
        opts.max_backoff = self.tuning.max_backoff;

        let name = t.name.clone();
        let runner = Runner::new(t, opts);
        if let Err(e) = runner.start().await {
            warn!(tunnel = %name, err = e.text(), "隧道启动失败");
            return Err(error::TUNNEL_START_FAILED.msg(e));
        }
        self.running.lock().unwrap().insert(id.to_string(), runner);
        self.notify_change();
        Ok(())
    }

    pub async fn stop(&self, id: &str) {
        let _op = self.op.lock().await;
        let runner = self.running.lock().unwrap().remove(id);
        if let Some(r) = runner {
            r.stop().await;
            self.notify_change();
        }
    }

    /// 启动全部隧道，返回启动失败的隧道名和原因。
    pub async fn start_all(&self) -> HashMap<String, Msg> {
        self.start_where(|_| true).await
    }

    /// 启动所有勾选了“随程序启动”的隧道。
    pub async fn start_auto(&self) -> HashMap<String, Msg> {
        self.start_where(|t| t.auto_start).await
    }

    async fn start_where(&self, pred: impl Fn(&Tunnel) -> bool) -> HashMap<String, Msg> {
        let list = match self.store.list() {
            Ok(l) => l,
            Err(e) => return HashMap::from([("*".to_string(), store_err_or(e, error::TUNNEL_LIST_FAILED).msg)]),
        };
        let mut failed = HashMap::new();
        for t in list.iter().filter(|t| pred(t)) {
            if let Err(e) = self.start(&t.id).await {
                failed.insert(t.name.clone(), e.msg);
            }
        }
        failed
    }

    pub async fn stop_all(&self) {
        let _op = self.op.lock().await;
        let all: Vec<_> = self.running.lock().unwrap().drain().map(|(_, r)| r).collect();
        let tasks: Vec<_> = all.into_iter().map(|r| tokio::spawn(async move { r.stop().await })).collect();
        for t in tasks {
            let _ = t.await;
        }
        self.notify_change();
    }

    pub fn running_count(&self) -> (usize, usize) {
        let running = self.running.lock().unwrap();
        let connected = running.values().filter(|r| r.status().state == TunnelState::Connected).count();
        (running.len(), connected)
    }

    /// 首次连接时推送指纹给前端确认，并在用户回答前挂起握手。
    fn trust_fn(&self, t: &Tunnel) -> TrustFn {
        let me = self.me.clone();
        let (id, name) = (t.id.clone(), t.name.clone());
        Arc::new(move |q: HostKeyQuery| {
            let me = me.clone();
            let (id, name) = (id.clone(), name.clone());
            Box::pin(async move {
                match me.upgrade() {
                    Some(svc) => svc.ask_trust(id, name, q).await,
                    None => false,
                }
            })
        })
    }

    async fn ask_trust(&self, tunnel_id: String, tunnel_name: String, q: HostKeyQuery) -> bool {
        let info = HostKeyPrompt {
            request_id: new_id(),
            tunnel_id,
            tunnel_name,
            host: q.host,
            key_type: q.key_type,
            fingerprint: q.fingerprint,
        };
        let (tx, rx) = oneshot::channel();
        let rid = info.request_id.clone();
        self.prompts.lock().unwrap().insert(rid.clone(), Prompt { info: info.clone(), answer: Some(tx) });

        info!(tunnel = %info.tunnel_name, host = %info.host, fingerprint = %info.fingerprint, "等待确认主机指纹");
        self.events.show_main_window();
        self.events.emit(events::HOST_KEY, serde_json::to_value(&info).unwrap_or_default());

        let ok = tokio::select! {
            r = rx => r.unwrap_or(false),
            _ = q.cancel.cancelled() => false,
            _ = tokio::time::sleep(self.tuning.prompt_timeout) => {
                warn!(tunnel = %info.tunnel_name, "主机指纹确认超时");
                false
            }
        };
        self.prompts.lock().unwrap().remove(&rid);
        ok
    }

    /// 尚未回答的指纹确认请求；窗口新建后前端会先拉一次，避免错过事件。
    pub fn pending_host_keys(&self) -> Vec<HostKeyPrompt> {
        self.prompts.lock().unwrap().values().map(|p| p.info.clone()).collect()
    }

    pub fn answer_host_key(&self, request_id: &str, accept: bool) -> ApiResult<()> {
        let tx = self.prompts.lock().unwrap().get_mut(request_id).and_then(|p| p.answer.take());
        match tx {
            Some(tx) => {
                let _ = tx.send(accept);
                Ok(())
            }
            None => Err(error::HOST_KEY_EXPIRED.err()),
        }
    }

    /// 所有运行中隧道的活动连接，按开始时间倒序。
    pub fn connections(&self) -> Vec<ConnectionView> {
        let mut out = Vec::new();
        for (id, r) in self.snapshot_running() {
            for c in r.stats().active() {
                out.push(ConnectionView {
                    id: format!("{id}:{}", c.id),
                    tunnel_id: id.clone(),
                    tunnel_name: r.def().name.clone(),
                    conn_id: c.id,
                    proto: c.proto,
                    client: c.client,
                    target: c.target,
                    start: c.start,
                    bytes_up: c.bytes_up,
                    bytes_down: c.bytes_down,
                });
            }
        }
        out.sort_by_key(|c| std::cmp::Reverse(c.start));
        out
    }

    pub fn close_connection(&self, tunnel_id: &str, conn_id: u64) -> ApiResult<()> {
        match self.get_running(tunnel_id) {
            Some(r) if r.stats().kill(conn_id) => Ok(()),
            _ => Err(error::CONN_NOT_FOUND.err()),
        }
    }

    pub fn close_all_connections(&self) -> usize {
        self.snapshot_running().values().map(|r| r.stats().kill_all()).sum()
    }
}
