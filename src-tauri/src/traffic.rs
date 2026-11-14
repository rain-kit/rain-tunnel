use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use chrono::Local;
use serde::Serialize;
use tokio::time::Instant;

use crate::core::stats::Collector;
use crate::core::tunnel::Runner;
use crate::model::{Traffic, TunnelTraffic};
use crate::usage::{UsageStore, UsageView};

/// 与前端流量图的点数一致。
const HISTORY_SIZE: usize = 90;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct TrafficPoint {
    /// 毫秒时间戳。
    pub t: i64,
    pub up: u64,
    pub down: u64,
}

/// 上次采样时某个隧道的计数。保留 Collector，隧道停止或重启后还能补记最后一段流量。
struct Last {
    stats: Arc<Collector>,
    up: u64,
    down: u64,
}

impl Last {
    fn delta(&self) -> (u64, u64) {
        let t = self.stats.totals();
        (t.up.saturating_sub(self.up), t.down.saturating_sub(self.down))
    }
}

#[derive(Default)]
struct State {
    last: HashMap<String, Last>,
    at: Option<Instant>,
    current: Traffic,
    history: VecDeque<TrafficPoint>,
}

/// 每秒计算一次各隧道速率，并把新增流量记入每日统计。
pub struct TrafficService {
    state: Mutex<State>,
    usage: UsageStore,
}

impl TrafficService {
    pub fn new(usage: UsageStore) -> Self {
        TrafficService { state: Mutex::new(State::default()), usage }
    }

    /// 计算距上次采样的速率。已停止的隧道不计入；重新启动后计数从零开始。
    pub fn sample(&self, running: &HashMap<String, Arc<Runner>>) -> Traffic {
        self.sample_stats(running.iter().map(|(id, r)| (id.clone(), r.stats().clone())))
    }

    fn sample_stats(&self, running: impl Iterator<Item = (String, Arc<Collector>)>) -> Traffic {
        let now = Instant::now();
        let mut st = self.state.lock().unwrap();
        let elapsed = st.at.map(|t| now.duration_since(t).as_secs_f64()).filter(|e| *e > 0.0).unwrap_or(1.0);
        let mut tr = Traffic {
            time: Some(Local::now()),
            memory: memory_stats::memory_stats().map(|m| m.physical_mem as u64).unwrap_or(0),
            ..Default::default()
        };
        let mut prev_all = std::mem::take(&mut st.last);
        let mut added = Vec::new();
        for (id, stats) in running {
            let totals = stats.totals();
            let prev = match prev_all.remove(&id) {
                Some(l) if Arc::ptr_eq(&l.stats, &stats) => (l.up, l.down),
                // 两次采样之间重启过：旧实例的最后一段流量也要记上。
                Some(l) => {
                    let (up, down) = l.delta();
                    added.push((id.clone(), up, down));
                    (0, 0)
                }
                None => (0, 0),
            };
            let (up, down) = (totals.up.saturating_sub(prev.0), totals.down.saturating_sub(prev.1));
            added.push((id.clone(), up, down));
            let tt = TunnelTraffic {
                up: (up as f64 / elapsed) as u64,
                down: (down as f64 / elapsed) as u64,
                total_up: totals.up,
                total_down: totals.down,
                active: totals.active,
            };
            tr.up += tt.up;
            tr.down += tt.down;
            tr.total_up += tt.total_up;
            tr.total_down += tt.total_down;
            tr.active += tt.active;
            tr.tunnels.insert(id.clone(), tt);
            st.last.insert(id, Last { stats, up: totals.up, down: totals.down });
        }
        for (id, l) in prev_all {
            let (up, down) = l.delta();
            added.push((id, up, down));
        }
        self.usage.add(Local::now().date_naive(), added);
        st.at = Some(now);
        st.current = tr.clone();
        if st.history.len() >= HISTORY_SIZE {
            st.history.pop_front();
        }
        st.history.push_back(TrafficPoint { t: Local::now().timestamp_millis(), up: tr.up, down: tr.down });
        tr
    }

    /// 最近 90 秒的总速率，窗口重新创建后用来恢复流量图。
    pub fn history(&self) -> Vec<TrafficPoint> {
        self.state.lock().unwrap().history.iter().copied().collect()
    }

    /// 最近一次采样结果，页面初次打开时使用，之后靠 traffic 事件更新。
    pub fn current(&self) -> Traffic {
        self.state.lock().unwrap().current.clone()
    }

    /// 累计流量和最近 days 天的每日流量。
    pub fn usage(&self, days: u64) -> UsageView {
        self.usage.view(Local::now().date_naive(), days)
    }

    /// 删除隧道时一并删除它的流量记录。
    pub fn remove(&self, id: &str) {
        self.state.lock().unwrap().last.remove(id);
        self.usage.remove(id);
    }

    pub fn save_usage(&self) {
        self.usage.save(Local::now().date_naive());
    }

    /// 退出前调用：补记上次采样后停止的隧道的流量，然后写盘。
    pub fn flush(&self) {
        let last = std::mem::take(&mut self.state.lock().unwrap().last);
        let added = last.into_iter().map(|(id, l)| {
            let (up, down) = l.delta();
            (id, up, down)
        });
        self.usage.add(Local::now().date_naive(), added);
        self.save_usage();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> (TrafficService, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("st-traffic-{}", crate::util::new_id()));
        (TrafficService::new(UsageStore::open(dir.join("traffic.json"))), dir)
    }

    fn total(s: &TrafficService, id: &str) -> (u64, u64) {
        s.usage(1).tunnels.get(id).map(|u| (u.total.up, u.total.down)).unwrap_or_default()
    }

    #[test]
    fn usage_counts_every_byte() {
        let (s, dir) = service();
        let c = Collector::new();
        let conn = c.open("local", "a", "b", || {});
        conn.add_up(5);
        conn.add_down(7);
        s.sample_stats([("t".to_string(), c.clone())].into_iter());
        assert_eq!(total(&s, "t"), (5, 7), "启动后首次采样前的流量也要记上");

        conn.add_up(1);
        s.sample_stats([("t".to_string(), c.clone())].into_iter());
        assert_eq!(total(&s, "t"), (6, 7));

        // 重启：旧实例在停止前又传了数据，新实例从零开始。
        conn.add_down(3);
        let c2 = Collector::new();
        c2.open("local", "a", "b", || {}).add_up(2);
        s.sample_stats([("t".to_string(), c2.clone())].into_iter());
        assert_eq!(total(&s, "t"), (8, 10));

        // 停止：最后一段流量在下次采样时补记。
        c2.open("local", "a", "b", || {}).add_down(4);
        s.sample_stats(std::iter::empty());
        assert_eq!(total(&s, "t"), (8, 14));
        s.sample_stats(std::iter::empty());
        assert_eq!(total(&s, "t"), (8, 14));

        s.remove("t");
        assert_eq!(total(&s, "t"), (0, 0));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn flush_records_stopped_tunnels() {
        let (s, dir) = service();
        let c = Collector::new();
        let conn = c.open("local", "a", "b", || {});
        s.sample_stats([("t".to_string(), c.clone())].into_iter());
        conn.add_up(9);
        s.flush();
        assert_eq!(total(&s, "t"), (9, 0));
        assert!(dir.join("traffic.json").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
