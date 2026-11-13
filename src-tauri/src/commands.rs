//! 前端通过 invoke 调用的接口，前端封装见 src/lib/api.ts。
//! 只返回数据和国际化 key（见 i18n::Msg、error::ApiError），文案由前端翻译。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tracing::info;

use crate::app::AppState;
use crate::config::{self, AppConfig};
use crate::error::{self, ApiResult};
use crate::i18n::{self, Msg};
use crate::model::{ConnectionView, HostKeyPrompt, LogEntry, SaveTunnelReq, Traffic, TunnelView};
use crate::traffic::TrafficPoint;
use crate::{tray, window};

type S<'a> = State<'a, AppState>;

#[tauri::command]
pub async fn tunnel_list(s: S<'_>) -> ApiResult<Vec<TunnelView>> {
    s.tunnels.list()
}

#[tauri::command]
pub async fn tunnel_get(s: S<'_>, id: String) -> ApiResult<TunnelView> {
    s.tunnels.get(&id)
}

#[tauri::command]
pub async fn tunnel_save(s: S<'_>, req: SaveTunnelReq) -> ApiResult<TunnelView> {
    s.tunnels.save(req).await
}

#[tauri::command]
pub async fn tunnel_duplicate(s: S<'_>, id: String) -> ApiResult<TunnelView> {
    s.tunnels.duplicate(&id).await
}

#[tauri::command]
pub async fn tunnel_delete(s: S<'_>, id: String) -> ApiResult<()> {
    s.tunnels.delete(&id).await
}

#[tauri::command]
pub async fn tunnel_reorder(s: S<'_>, ids: Vec<String>) -> ApiResult<()> {
    s.tunnels.reorder(&ids)
}

#[tauri::command]
pub async fn tunnel_start(s: S<'_>, id: String) -> ApiResult<()> {
    s.tunnels.start(&id).await
}

#[tauri::command]
pub async fn tunnel_stop(s: S<'_>, id: String) -> ApiResult<()> {
    s.tunnels.stop(&id).await;
    Ok(())
}

/// 返回启动失败的隧道名 → 原因。
#[tauri::command]
pub async fn tunnel_start_all(s: S<'_>) -> ApiResult<HashMap<String, Msg>> {
    Ok(s.tunnels.start_all().await)
}

#[tauri::command]
pub async fn tunnel_stop_all(s: S<'_>) -> ApiResult<()> {
    s.tunnels.stop_all().await;
    Ok(())
}

#[tauri::command]
pub async fn hostkey_pending(s: S<'_>) -> ApiResult<Vec<HostKeyPrompt>> {
    Ok(s.tunnels.pending_host_keys())
}

#[tauri::command]
pub async fn hostkey_answer(s: S<'_>, request_id: String, accept: bool) -> ApiResult<()> {
    s.tunnels.answer_host_key(&request_id, accept)
}

#[tauri::command]
pub async fn conn_list(s: S<'_>) -> ApiResult<Vec<ConnectionView>> {
    Ok(s.tunnels.connections())
}

#[tauri::command]
pub async fn conn_close(s: S<'_>, tunnel_id: String, conn_id: u64) -> ApiResult<()> {
    s.tunnels.close_connection(&tunnel_id, conn_id)
}

#[tauri::command]
pub async fn conn_close_all(s: S<'_>) -> ApiResult<usize> {
    Ok(s.tunnels.close_all_connections())
}

#[tauri::command]
pub async fn traffic_current(s: S<'_>) -> ApiResult<Traffic> {
    Ok(s.traffic.current())
}

#[tauri::command]
pub async fn traffic_history(s: S<'_>) -> ApiResult<Vec<TrafficPoint>> {
    Ok(s.traffic.history())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    log_level: String,
    theme: String,
    language: String,
    system_language: &'static str,
    close_to_tray: bool,
    silent_start: bool,
    auto_launch: bool,
    data_dir: String,
    log_dir: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    log_level: String,
    theme: String,
    language: String,
    close_to_tray: bool,
    silent_start: bool,
}

fn settings_view(app: &AppHandle, s: &AppState) -> SettingsView {
    let cfg = s.config.lock().unwrap().clone();
    SettingsView {
        log_level: cfg.system.log_level,
        theme: cfg.ui.theme,
        language: cfg.ui.language,
        system_language: i18n::system().tag(),
        close_to_tray: cfg.app.close_to_tray,
        silent_start: cfg.app.silent_start,
        auto_launch: app.autolaunch().is_enabled().unwrap_or(false),
        data_dir: s.data_dir.display().to_string(),
        log_dir: s.log_dir().display().to_string(),
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub async fn settings_get(app: AppHandle, s: S<'_>) -> ApiResult<SettingsView> {
    Ok(settings_view(&app, &s))
}

#[tauri::command]
pub async fn settings_save(app: AppHandle, s: S<'_>, input: SettingsInput) -> ApiResult<SettingsView> {
    let mut cfg: AppConfig = s.config.lock().unwrap().clone();
    cfg.system.log_level = input.log_level;
    cfg.ui.theme = input.theme;
    cfg.ui.language = input.language;
    cfg.app.close_to_tray = input.close_to_tray;
    cfg.app.silent_start = input.silent_start;
    cfg.validate().map_err(|e| error::PARAM_INVALID.msg(e))?;
    config::save(&s.data_dir, &cfg).map_err(|e| error::SETTING_SAVE_FAILED.cause(e))?;

    s.level.set(&cfg.system.log_level);
    window::apply_theme(&app, &cfg.ui.theme);
    i18n::set(&cfg.ui.language);
    tray::refresh_soon(&app);
    info!(level = %cfg.system.log_level, theme = %cfg.ui.theme, language = %cfg.ui.language, close_to_tray = cfg.app.close_to_tray, silent_start = cfg.app.silent_start, "设置已保存");
    *s.config.lock().unwrap() = cfg;
    Ok(settings_view(&app, &s))
}

#[tauri::command]
pub async fn settings_set_autolaunch(app: AppHandle, s: S<'_>, enable: bool) -> ApiResult<SettingsView> {
    let al = app.autolaunch();
    let r = if enable { al.enable() } else { al.disable() };
    r.map_err(|e| error::AUTOSTART_FAILED.cause(e))?;
    info!(enable, "开机自启已更新");
    Ok(settings_view(&app, &s))
}

#[tauri::command]
pub async fn open_data_dir(app: AppHandle, s: S<'_>) -> ApiResult<()> {
    open_dir(&app, &s.data_dir)
}

#[tauri::command]
pub async fn open_log_dir(app: AppHandle, s: S<'_>) -> ApiResult<()> {
    let dir = s.log_dir();
    let _ = std::fs::create_dir_all(&dir);
    open_dir(&app, &dir)
}

fn open_dir(app: &AppHandle, dir: &std::path::Path) -> ApiResult<()> {
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| error::OPEN_DIR_FAILED.cause(e))
}

/// 选择私钥文件，取消时返回空字符串。
#[tauri::command]
pub async fn pick_key_file(app: AppHandle) -> ApiResult<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut dlg = app.dialog().file().set_title(i18n::pick(["选择 SSH 私钥", "選擇 SSH 私鑰", "Choose SSH private key"]));
    if let Some(home) = dirs::home_dir() {
        let ssh = home.join(".ssh");
        dlg = dlg.set_directory(if ssh.is_dir() { ssh } else { home });
    }
    dlg.pick_file(move |p| {
        let _ = tx.send(p);
    });
    let picked = rx.await.ok().flatten();
    Ok(picked.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()).unwrap_or_default())
}

#[tauri::command]
pub async fn logs_list() -> ApiResult<Vec<LogEntry>> {
    Ok(crate::logging::ring().list())
}

#[tauri::command]
pub async fn logs_clear() -> ApiResult<()> {
    crate::logging::ring().clear();
    Ok(())
}
