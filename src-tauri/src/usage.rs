use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::util::write_file_atomic;

/// 每日记录保留的天数，累计流量不受影响。
const KEEP_DAYS: u64 = 366;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bytes {
    pub up: u64,
    pub down: u64,
}

impl Bytes {
    fn add(&mut self, up: u64, down: u64) {
        self.up = self.up.saturating_add(up);
        self.down = self.down.saturating_add(down);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TunnelUsage {
    pub total: Bytes,
    /// 按本地日期，键为 YYYY-MM-DD。
    pub days: BTreeMap<NaiveDate, Bytes>,
}

#[derive(Default, Serialize, Deserialize)]
struct UsageFile {
    #[serde(default)]
    tunnels: HashMap<String, TunnelUsage>,
}

#[derive(Debug, Serialize)]
pub struct UsageView {
    pub today: NaiveDate,
    pub tunnels: HashMap<String, TunnelUsage>,
}

#[derive(Default)]
struct Inner {
    data: UsageFile,
    dirty: bool,
}

/// 各隧道每天的流量和累计流量，保存在数据目录下的 traffic.json。
pub struct UsageStore {
    path: PathBuf,
    inner: Mutex<Inner>,
}

impl UsageStore {
    /// 读取已有记录；文件损坏时改名为 traffic.json.bak 后重新开始，避免被覆盖。
    pub fn open(path: PathBuf) -> Self {
        let data = match std::fs::read(&path) {
            Ok(d) if d.iter().all(u8::is_ascii_whitespace) => UsageFile::default(),
            Ok(d) => serde_json::from_slice(&d).unwrap_or_else(|e| {
                warn!(err = %e, "traffic.json 格式错误，已备份后重新记录");
                let _ = std::fs::rename(&path, path.with_extension("json.bak"));
                UsageFile::default()
            }),
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!(err = %e, "读取 traffic.json 失败");
                }
                UsageFile::default()
            }
        };
        UsageStore { path, inner: Mutex::new(Inner { data, dirty: false }) }
    }

    /// 把各隧道的新增流量（ID、上传、下载）记到指定日期。
    pub fn add(&self, day: NaiveDate, items: impl IntoIterator<Item = (String, u64, u64)>) {
        let mut inner = self.inner.lock().unwrap();
        for (id, up, down) in items {
            if up == 0 && down == 0 {
                continue;
            }
            let u = inner.data.tunnels.entry(id).or_default();
            u.total.add(up, down);
            u.days.entry(day).or_default().add(up, down);
            inner.dirty = true;
        }
    }

    pub fn remove(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        if inner.data.tunnels.remove(id).is_some() {
            inner.dirty = true;
        }
    }

    /// 累计流量和最近 days 天（含今天）的每日流量。
    pub fn view(&self, today: NaiveDate, days: u64) -> UsageView {
        let from = today.checked_sub_days(Days::new(days.saturating_sub(1))).unwrap_or(NaiveDate::MIN);
        let inner = self.inner.lock().unwrap();
        let tunnels = inner
            .data
            .tunnels
            .iter()
            .map(|(id, u)| {
                let days = u.days.range(from..=today).map(|(d, b)| (*d, *b)).collect();
                (id.clone(), TunnelUsage { total: u.total, days })
            })
            .collect();
        UsageView { today, tunnels }
    }

    /// 有改动时写盘，并清理超过保留期的每日记录。
    pub fn save(&self, today: NaiveDate) {
        let data = {
            let mut inner = self.inner.lock().unwrap();
            if !inner.dirty {
                return;
            }
            let cutoff = today.checked_sub_days(Days::new(KEEP_DAYS - 1)).unwrap_or(NaiveDate::MIN);
            for u in inner.data.tunnels.values_mut() {
                u.days.retain(|d, _| *d >= cutoff);
            }
            inner.dirty = false;
            match serde_json::to_vec(&inner.data) {
                Ok(d) => d,
                Err(e) => {
                    warn!(err = %e, "序列化流量记录失败");
                    return;
                }
            }
        };
        if let Err(e) = write_file_atomic(&self.path, &data) {
            warn!(err = %e, "保存流量记录失败");
            self.inner.lock().unwrap().dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn record_view_and_persist() {
        let dir = std::env::temp_dir().join(format!("st-usage-{}", crate::util::new_id()));
        let path = dir.join("traffic.json");
        let store = UsageStore::open(path.clone());
        store.add(day("2026-11-13"), [("a".into(), 10, 20), ("b".into(), 0, 0)]);
        store.add(day("2026-11-14"), [("a".into(), 1, 2)]);
        store.add(day("2026-11-14"), [("a".into(), 1, 2)]);

        let v = store.view(day("2026-11-14"), 1);
        assert!(!v.tunnels.contains_key("b"), "没有流量的隧道不记录");
        let a = &v.tunnels["a"];
        assert_eq!(a.total, Bytes { up: 12, down: 24 });
        assert_eq!(a.days.len(), 1, "只返回范围内的日期");
        assert_eq!(a.days[&day("2026-11-14")], Bytes { up: 2, down: 4 });

        store.save(day("2026-11-14"));
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"2026-11-13\""));
        let reopened = UsageStore::open(path.clone());
        assert_eq!(reopened.view(day("2026-11-14"), 30).tunnels, store.view(day("2026-11-14"), 30).tunnels);

        reopened.remove("a");
        assert!(reopened.view(day("2026-11-14"), 30).tunnels.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_old_days_keep_total() {
        let dir = std::env::temp_dir().join(format!("st-usage-{}", crate::util::new_id()));
        let store = UsageStore::open(dir.join("traffic.json"));
        store.add(day("2025-01-01"), [("a".into(), 5, 5)]);
        store.add(day("2026-11-14"), [("a".into(), 1, 1)]);
        store.save(day("2026-11-14"));
        let v = store.view(day("2026-11-14"), 10000);
        assert_eq!(v.tunnels["a"].days.len(), 1);
        assert_eq!(v.tunnels["a"].total, Bytes { up: 6, down: 6 });
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn broken_file_is_backed_up() {
        let dir = std::env::temp_dir().join(format!("st-usage-{}", crate::util::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("traffic.json");
        std::fs::write(&path, "{oops").unwrap();
        let store = UsageStore::open(path.clone());
        assert!(store.view(day("2026-11-14"), 1).tunnels.is_empty());
        assert!(dir.join("traffic.json.bak").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
