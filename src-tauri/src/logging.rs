use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::Local;
use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{reload, Layer, Registry};

use crate::model::LogEntry;

const RING_SIZE: usize = 2000;
const FILE_MAX_SIZE: u64 = 10 * 1024 * 1024;
const FILE_BACKUPS: usize = 3;
pub const LOG_FILE: &str = "rain-tunnel.log";

type Listener = Box<dyn Fn(&LogEntry) + Send + Sync>;

/// 最近的日志，供“日志”页面查看。
pub struct LogRing {
    inner: Mutex<(VecDeque<LogEntry>, u64)>,
    listener: OnceLock<Listener>,
}

thread_local! {
    static IN_LISTENER: Cell<bool> = const { Cell::new(false) };
}

impl LogRing {
    fn new() -> Self {
        LogRing { inner: Mutex::new((VecDeque::with_capacity(RING_SIZE), 0)), listener: OnceLock::new() }
    }

    /// 每条新日志都会调用 f（用于推送给前端），只能设置一次。
    pub fn set_listener(&self, f: impl Fn(&LogEntry) + Send + Sync + 'static) {
        let _ = self.listener.set(Box::new(f));
    }

    pub fn push(&self, level: &str, message: String, fields: String) {
        let entry = {
            let mut g = self.inner.lock().unwrap();
            g.1 += 1;
            let e = LogEntry { seq: g.1, time: Local::now(), level: level.to_string(), message, fields };
            if g.0.len() >= RING_SIZE {
                g.0.pop_front();
            }
            g.0.push_back(e.clone());
            e
        };
        if let Some(f) = self.listener.get() {
            // 推送过程中产生的日志不再推送，避免递归。
            if !IN_LISTENER.with(|c| c.replace(true)) {
                f(&entry);
                IN_LISTENER.with(|c| c.set(false));
            }
        }
    }

    pub fn list(&self) -> Vec<LogEntry> {
        self.inner.lock().unwrap().0.iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().0.clear();
    }
}

pub fn ring() -> &'static LogRing {
    static RING: OnceLock<LogRing> = OnceLock::new();
    RING.get_or_init(LogRing::new)
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields.push((field.name().to_string(), value.to_string()));
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let s = format!("{value:?}");
        if field.name() == "message" {
            self.message = s;
        } else {
            self.fields.push((field.name().to_string(), s));
        }
    }
}

struct RingLayer;

impl<S: Subscriber> Layer<S> for RingLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut v = FieldVisitor::default();
        event.record(&mut v);
        v.fields.sort();
        let mut fields = String::new();
        for (k, val) in &v.fields {
            if !fields.is_empty() {
                fields.push(' ');
            }
            if val.contains(' ') {
                let _ = write!(fields, "{k}={val:?}");
            } else {
                let _ = write!(fields, "{k}={val}");
            }
        }
        ring().push(level_name(*event.metadata().level()), v.message, fields);
    }
}

fn level_name(l: Level) -> &'static str {
    match l {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        _ => "debug",
    }
}

/// 按大小滚动的日志文件：超过 10MB 时改名为 .1，最多保留 3 个旧文件。
pub struct RotatingFile {
    path: PathBuf,
    state: Mutex<Option<(File, u64)>>,
}

impl RotatingFile {
    pub fn new(path: PathBuf) -> Self {
        RotatingFile { path, state: Mutex::new(None) }
    }

    fn open(&self) -> io::Result<(File, u64)> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let f = OpenOptions::new().create(true).append(true).open(&self.path)?;
        let size = f.metadata().map(|m| m.len()).unwrap_or(0);
        Ok((f, size))
    }

    fn rotate(&self) {
        for i in (1..FILE_BACKUPS).rev() {
            let _ = fs::rename(backup(&self.path, i), backup(&self.path, i + 1));
        }
        let _ = fs::rename(&self.path, backup(&self.path, 1));
    }

    fn write_all(&self, buf: &[u8]) -> io::Result<()> {
        let mut st = self.state.lock().unwrap();
        if st.as_ref().is_some_and(|(_, size)| *size + buf.len() as u64 > FILE_MAX_SIZE) {
            *st = None;
            self.rotate();
        }
        if st.is_none() {
            *st = Some(self.open()?);
        }
        let (f, size) = st.as_mut().unwrap();
        f.write_all(buf)?;
        *size += buf.len() as u64;
        Ok(())
    }
}

fn backup(path: &Path, i: usize) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(format!(".{i}"));
    PathBuf::from(s)
}

pub struct FileWriter<'a>(&'a RotatingFile);

impl Write for FileWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for RotatingFile {
    type Writer = FileWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        FileWriter(self)
    }
}

pub fn parse_level(s: &str) -> LevelFilter {
    match s {
        "debug" => LevelFilter::DEBUG,
        "warn" => LevelFilter::WARN,
        "error" => LevelFilter::ERROR,
        _ => LevelFilter::INFO,
    }
}

/// 本程序按配置级别输出；第三方库最多输出到 warn，避免 russh 等刷屏。
fn targets(level: &str) -> Targets {
    let lvl = parse_level(level);
    Targets::new().with_target(env!("CARGO_CRATE_NAME"), lvl).with_default(lvl.min(LevelFilter::WARN))
}

#[derive(Clone)]
pub struct LevelHandle(reload::Handle<Targets, Registry>);

impl LevelHandle {
    pub fn set(&self, level: &str) {
        let _ = self.0.reload(targets(level));
    }
}

/// 初始化全局日志：写入 logs/rain-tunnel.log（JSON 行）、内存环形缓冲，调试版同时输出到控制台。
pub fn init(log_dir: &Path, level: &str) -> LevelHandle {
    let (filter, handle) = reload::Layer::new(targets(level));
    let file = RotatingFile::new(log_dir.join(LOG_FILE));
    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
        .with_current_span(false)
        .with_span_list(false)
        .with_writer(file);
    let console = cfg!(debug_assertions).then(|| {
        tracing_subscriber::fmt::layer()
            .with_timer(tracing_subscriber::fmt::time::ChronoLocal::new("%H:%M:%S%.3f".into()))
            .with_writer(io::stderr)
    });
    let _ = tracing_subscriber::registry().with(filter).with(file_layer).with(console).with(RingLayer).try_init();
    LevelHandle(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_keeps_latest() {
        let r = LogRing::new();
        for i in 0..RING_SIZE + 5 {
            r.push("info", format!("m{i}"), String::new());
        }
        let list = r.list();
        assert_eq!(list.len(), RING_SIZE);
        assert_eq!(list[0].message, "m5");
        assert_eq!(list.last().unwrap().seq, (RING_SIZE + 5) as u64);
    }

    #[test]
    fn rotating_file_rotates() {
        let dir = std::env::temp_dir().join(format!("raintunnel-log-{}", crate::util::new_id()));
        let f = RotatingFile::new(dir.join("a.log"));
        let chunk = vec![b'x'; 1024 * 1024];
        for _ in 0..12 {
            f.write_all(&chunk).unwrap();
        }
        assert!(dir.join("a.log.1").exists());
        assert!(fs::metadata(dir.join("a.log")).unwrap().len() <= FILE_MAX_SIZE);
        let _ = fs::remove_dir_all(dir);
    }
}
