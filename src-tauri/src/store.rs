use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::model::Tunnel;
use crate::util::write_file_atomic;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("记录不存在")]
    NotFound,
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("tunnels.json 格式错误: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Serialize, Deserialize, Default)]
struct TunnelFile {
    #[serde(default)]
    tunnels: Vec<Tunnel>,
}

/// 把隧道列表保存在数据目录下的 tunnels.json（与 v1 格式相同）；密码类字段不落盘。
pub struct TunnelStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl TunnelStore {
    pub fn new(path: PathBuf) -> Self {
        TunnelStore { path, lock: Mutex::new(()) }
    }

    fn load(&self) -> Result<Vec<Tunnel>, StoreError> {
        let data = match std::fs::read(&self.path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        if data.iter().all(u8::is_ascii_whitespace) {
            return Ok(Vec::new());
        }
        let mut f: TunnelFile = serde_json::from_slice(&data)?;
        for t in &mut f.tunnels {
            t.normalize();
        }
        Ok(f.tunnels)
    }

    fn save(&self, list: &[Tunnel]) -> Result<(), StoreError> {
        let tunnels = list
            .iter()
            .cloned()
            .map(|mut t| {
                t.clear_secrets();
                t
            })
            .collect();
        let data = serde_json::to_vec_pretty(&TunnelFile { tunnels })?;
        write_file_atomic(&self.path, &data)?;
        Ok(())
    }

    /// 全部隧道（不含密码），按保存顺序。
    pub fn list(&self) -> Result<Vec<Tunnel>, StoreError> {
        let _g = self.lock.lock().unwrap();
        self.load()
    }

    pub fn get(&self, id: &str) -> Result<Tunnel, StoreError> {
        self.list()?.into_iter().find(|t| t.id == id).ok_or(StoreError::NotFound)
    }

    /// 按 ID 新增或覆盖。
    pub fn save_one(&self, t: Tunnel) -> Result<(), StoreError> {
        let _g = self.lock.lock().unwrap();
        let mut list = self.load()?;
        match list.iter_mut().find(|x| x.id == t.id) {
            Some(slot) => *slot = t,
            None => list.push(t),
        }
        self.save(&list)
    }

    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        let _g = self.lock.lock().unwrap();
        let mut list = self.load()?;
        let n = list.len();
        list.retain(|x| x.id != id);
        if list.len() == n {
            return Err(StoreError::NotFound);
        }
        self.save(&list)
    }

    /// 按给定 ID 顺序重排，未列出的隧道保持原相对顺序排在最后。
    pub fn reorder(&self, ids: &[String]) -> Result<(), StoreError> {
        let _g = self.lock.lock().unwrap();
        let mut list = self.load()?;
        let rank: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();
        list.sort_by_key(|t| rank.get(t.id.as_str()).copied().unwrap_or(usize::MAX));
        self.save(&list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tunnel(id: &str) -> Tunnel {
        let mut t = Tunnel {
            id: id.into(),
            ssh_host: "h".into(),
            ssh_user: "u".into(),
            password: "pw".into(),
            ..Default::default()
        };
        t.normalize();
        t
    }

    #[test]
    fn crud_and_reorder() {
        let dir = std::env::temp_dir().join(format!("st-store-{}", crate::util::new_id()));
        let store = TunnelStore::new(dir.join("tunnels.json"));
        assert!(store.list().unwrap().is_empty());

        for id in ["a", "b", "c"] {
            store.save_one(tunnel(id)).unwrap();
        }
        let raw = std::fs::read_to_string(dir.join("tunnels.json")).unwrap();
        assert!(!raw.contains("pw"), "密码不能写入 tunnels.json");

        let mut b = store.get("b").unwrap();
        b.name = "renamed".into();
        store.save_one(b).unwrap();
        assert_eq!(store.get("b").unwrap().name, "renamed");

        store.reorder(&["c".into(), "a".into()]).unwrap();
        let ids: Vec<_> = store.list().unwrap().into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["c", "a", "b"]);

        store.delete("a").unwrap();
        assert!(matches!(store.delete("a"), Err(StoreError::NotFound)));
        assert!(matches!(store.get("zzz"), Err(StoreError::NotFound)));
        let _ = std::fs::remove_dir_all(dir);
    }
}
