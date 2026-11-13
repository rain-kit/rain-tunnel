/// 推送给前端的事件名，前端在 src/services/events.ts 中订阅。
pub const TUNNEL_STATUS: &str = "tunnel:status";
pub const TUNNEL_CHANGED: &str = "tunnel:changed";
pub const TRAFFIC: &str = "traffic";
pub const LOG: &str = "log";
pub const HOST_KEY: &str = "hostkey:prompt";
pub const UPDATE: &str = "update:status";
pub const UPDATE_PROGRESS: &str = "update:progress";
pub const UPDATE_OPEN: &str = "update:open";

/// 业务层通过它通知界面，测试中可以替换成记录或空实现。
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
    /// 显示主窗口（窗口已销毁时重新创建）。
    fn show_main_window(&self);
}

pub struct NoopSink;

impl EventSink for NoopSink {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
    fn show_main_window(&self) {}
}
