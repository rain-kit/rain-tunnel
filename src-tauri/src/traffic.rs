use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use chrono::Local;
use serde::Serialize;
use tokio::time::Instant;

use crate::core::tunnel::Runner;
use crate::model::{Traffic, TunnelTraffic};

/// 与前端流量图的点数一致。
const HISTORY_SIZE: usize = 90;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct TrafficPoint {
    /// 毫秒时间戳。
    pub t: i64,
    pub up: u64,
    pub down: u64,
}

#[derive(Default)]
struct State {
    last: HashMap<String, (u64, u64)>,
    at: Option<Instant>,
    current: Traffic,
    history: VecDeque<TrafficPoint>,
}

/// 每秒计算一次各隧道速率。
#[derive(Default)]
pub struct TrafficService {
    state: Mutex<State>,
}

impl TrafficService {
    /// 计算距上次采样的速率。已停止的隧道不计入；重新启动后计数从零开始。
    pub fn sample(&self, running: &HashMap<String, Arc<Runner>>) -> Traffic {
        let now = Instant::now();
        let mut st = self.state.lock().unwrap();
        let elapsed = st.at.map(|t| now.duration_since(t).as_secs_f64()).filter(|e| *e > 0.0).unwrap_or(1.0);
        let mut tr = Traffic {
            time: Some(Local::now()),
            memory: memory_stats::memory_stats().map(|m| m.physical_mem as u64).unwrap_or(0),
            ..Default::default()
        };
        let mut next = HashMap::with_capacity(running.len());
        for (id, r) in running {
            let totals = r.stats().totals();
            let prev = match st.last.get(id) {
                Some(&(up, down)) if totals.up >= up && totals.down >= down => (up, down),
                _ => (totals.up, totals.down),
            };
            let tt = TunnelTraffic {
                up: ((totals.up - prev.0) as f64 / elapsed) as u64,
                down: ((totals.down - prev.1) as f64 / elapsed) as u64,
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
            next.insert(id.clone(), (totals.up, totals.down));
        }
        st.last = next;
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
}
