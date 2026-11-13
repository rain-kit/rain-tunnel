use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Local};

const RECENT_LIMIT: usize = 100;

type KillFn = Box<dyn Fn() + Send + Sync>;

/// 汇总一个隧道的流量和连接信息，所有方法都可并发调用。
#[derive(Default)]
pub struct Collector {
    bytes_up: AtomicU64,
    bytes_down: AtomicU64,
    total_conns: AtomicU64,
    next_id: AtomicU64,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    active: HashMap<u64, Arc<ConnState>>,
    recent: VecDeque<ConnInfo>,
}

struct ConnState {
    id: u64,
    proto: String,
    client: String,
    target: String,
    start: DateTime<Local>,
    up: AtomicU64,
    down: AtomicU64,
    kill: KillFn,
}

impl ConnState {
    fn info(&self) -> ConnInfo {
        ConnInfo {
            id: self.id,
            proto: self.proto.clone(),
            client: self.client.clone(),
            target: self.target.clone(),
            start: self.start,
            end: None,
            bytes_up: self.up.load(Ordering::Relaxed),
            bytes_down: self.down.load(Ordering::Relaxed),
            error: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnInfo {
    pub id: u64,
    pub proto: String,
    pub client: String,
    pub target: String,
    pub start: DateTime<Local>,
    pub end: Option<DateTime<Local>>,
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub error: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Totals {
    pub up: u64,
    pub down: u64,
    pub active: usize,
}

/// 一条登记中的连接；drop 时若还没 close，按正常结束记入最近列表。
pub struct Conn {
    collector: Arc<Collector>,
    state: Arc<ConnState>,
    closed: bool,
}

impl Collector {
    pub fn new() -> Arc<Self> {
        Arc::new(Collector::default())
    }

    /// 登记一条新连接；kill 用于从界面上强制断开该连接。
    pub fn open(
        self: &Arc<Self>,
        proto: &str,
        client: impl Into<String>,
        target: impl Into<String>,
        kill: impl Fn() + Send + Sync + 'static,
    ) -> Conn {
        let state = Arc::new(ConnState {
            id: self.next_id.fetch_add(1, Ordering::Relaxed) + 1,
            proto: proto.to_string(),
            client: client.into(),
            target: target.into(),
            start: Local::now(),
            up: AtomicU64::new(0),
            down: AtomicU64::new(0),
            kill: Box::new(kill),
        });
        self.total_conns.fetch_add(1, Ordering::Relaxed);
        self.inner.lock().unwrap().active.insert(state.id, state.clone());
        Conn { collector: self.clone(), state, closed: false }
    }

    pub fn totals(&self) -> Totals {
        Totals {
            up: self.bytes_up.load(Ordering::Relaxed),
            down: self.bytes_down.load(Ordering::Relaxed),
            active: self.inner.lock().unwrap().active.len(),
        }
    }

    pub fn total_conns(&self) -> u64 {
        self.total_conns.load(Ordering::Relaxed)
    }

    /// 活动连接，按 ID 升序。
    pub fn active(&self) -> Vec<ConnInfo> {
        let mut list: Vec<ConnInfo> = self.inner.lock().unwrap().active.values().map(|c| c.info()).collect();
        list.sort_by_key(|c| c.id);
        list
    }

    pub fn recent(&self) -> Vec<ConnInfo> {
        self.inner.lock().unwrap().recent.iter().cloned().collect()
    }

    /// 强制断开指定连接，连接不存在时返回 false。
    pub fn kill(&self, id: u64) -> bool {
        let state = self.inner.lock().unwrap().active.get(&id).cloned();
        match state {
            Some(s) => {
                (s.kill)();
                true
            }
            None => false,
        }
    }

    pub fn kill_all(&self) -> usize {
        let all: Vec<_> = self.inner.lock().unwrap().active.values().cloned().collect();
        for s in &all {
            (s.kill)();
        }
        all.len()
    }
}

impl Conn {
    pub fn id(&self) -> u64 {
        self.state.id
    }

    pub fn add_up(&self, n: u64) {
        self.state.up.fetch_add(n, Ordering::Relaxed);
        self.collector.bytes_up.fetch_add(n, Ordering::Relaxed);
    }

    pub fn add_down(&self, n: u64) {
        self.state.down.fetch_add(n, Ordering::Relaxed);
        self.collector.bytes_down.fetch_add(n, Ordering::Relaxed);
    }

    /// 结束连接并记入最近连接列表。
    pub fn close(mut self, err: Option<String>) {
        self.finish(err);
    }

    fn finish(&mut self, err: Option<String>) {
        if self.closed {
            return;
        }
        self.closed = true;
        let mut info = self.state.info();
        info.end = Some(Local::now());
        info.error = err.unwrap_or_default();
        let mut inner = self.collector.inner.lock().unwrap();
        inner.active.remove(&self.state.id);
        inner.recent.push_back(info);
        while inner.recent.len() > RECENT_LIMIT {
            inner.recent.pop_front();
        }
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.finish(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn open_count_close() {
        let c = Collector::new();
        let killed = Arc::new(AtomicBool::new(false));
        let k = killed.clone();
        let conn = c.open("socks5", "127.0.0.1:1", "example.com:443", move || k.store(true, Ordering::SeqCst));
        conn.add_up(10);
        conn.add_down(20);
        let t = c.totals();
        assert_eq!((t.up, t.down, t.active), (10, 20, 1));
        assert!(c.kill(conn.id()));
        assert!(killed.load(Ordering::SeqCst));
        conn.close(Some("boom".into()));
        assert_eq!(c.totals().active, 0);
        assert_eq!(c.recent()[0].error, "boom");
        assert!(!c.kill(1));
    }

    #[test]
    fn recent_is_bounded() {
        let c = Collector::new();
        for _ in 0..RECENT_LIMIT + 10 {
            drop(c.open("local", "a", "b", || {}));
        }
        assert_eq!(c.recent().len(), RECENT_LIMIT);
        assert_eq!(c.total_conns(), (RECENT_LIMIT + 10) as u64);
    }
}
