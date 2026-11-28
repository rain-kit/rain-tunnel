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
use crate::update::{self, UpdateInfo};
use crate::usage::UsageView;
use crate::{shortcut, tray, window};

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
    s.tunnels.delete(&id).await?;
    s.traffic.remove(&id);
    Ok(())
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

/// 各隧道的累计流量和最近 days 天（含今天）的每日流量。
#[tauri::command]
pub async fn traffic_usage(s: S<'_>, days: u32) -> ApiResult<UsageView> {
    Ok(s.traffic.usage(days.clamp(1, 366) as u64))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    theme: String,
    language: String,
    system_language: &'static str,
    terminal: String,
    close_to_tray: bool,
    minimize_action: String,
    silent_start: bool,
    auto_check_update: bool,
    tools_shortcut_enabled: bool,
    tools_shortcut: String,
    default_tools_shortcut: &'static str,
    /// 快捷键注册失败的原因
    tools_shortcut_error: Option<Msg>,
    auto_launch: bool,
    version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    theme: String,
    language: String,
    terminal: String,
    close_to_tray: bool,
    minimize_action: String,
    silent_start: bool,
    auto_check_update: bool,
    tools_shortcut_enabled: bool,
    tools_shortcut: String,
}

fn settings_view(app: &AppHandle, s: &AppState) -> SettingsView {
    let cfg = s.config.lock().unwrap().clone();
    SettingsView {
        theme: cfg.ui.theme,
        language: cfg.ui.language,
        system_language: i18n::system().tag(),
        terminal: cfg.ui.terminal,
        close_to_tray: cfg.app.close_to_tray,
        minimize_action: cfg.app.minimize_action,
        silent_start: cfg.app.silent_start,
        auto_check_update: cfg.app.auto_check_update,
        tools_shortcut_enabled: cfg.app.tools_shortcut_enabled,
        tools_shortcut: cfg.app.tools_shortcut,
        default_tools_shortcut: shortcut::default_shortcut(),
        tools_shortcut_error: shortcut::last_error(),
        auto_launch: app.autolaunch().is_enabled().unwrap_or(false),
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
    cfg.ui.theme = input.theme;
    cfg.ui.language = input.language;
    cfg.ui.terminal = input.terminal;
    cfg.app.close_to_tray = input.close_to_tray;
    cfg.app.minimize_action = input.minimize_action;
    cfg.app.silent_start = input.silent_start;
    let check_enabled = input.auto_check_update && !cfg.app.auto_check_update;
    cfg.app.auto_check_update = input.auto_check_update;
    let old_shortcut = (cfg.app.tools_shortcut_enabled, cfg.app.tools_shortcut.clone());
    cfg.app.tools_shortcut_enabled = input.tools_shortcut_enabled;
    cfg.app.tools_shortcut = input.tools_shortcut.trim().to_string();
    cfg.validate().map_err(|e| error::PARAM_INVALID.msg(e))?;
    let shortcut_changed = old_shortcut != (cfg.app.tools_shortcut_enabled, cfg.app.tools_shortcut.clone());
    if shortcut_changed {
        // 新快捷键注册不上时恢复原来的，设置不保存。
        if let Err(e) = shortcut::apply(&app, cfg.app.tools_shortcut_enabled, &cfg.app.tools_shortcut) {
            let _ = shortcut::apply(&app, old_shortcut.0, &old_shortcut.1);
            return Err(e);
        }
    }
    if let Err(e) = config::save(&s.data_dir, &cfg) {
        if shortcut_changed {
            let _ = shortcut::apply(&app, old_shortcut.0, &old_shortcut.1);
        }
        return Err(error::SETTING_SAVE_FAILED.cause(e));
    }

    window::apply_theme(&app, &cfg.ui.theme);
    i18n::set(&cfg.ui.language);
    tray::refresh_soon(&app);
    info!(theme = %cfg.ui.theme, language = %cfg.ui.language, terminal = %cfg.ui.terminal, close_to_tray = cfg.app.close_to_tray, minimize_action = %cfg.app.minimize_action, silent_start = cfg.app.silent_start, auto_check_update = cfg.app.auto_check_update, tools_shortcut_enabled = cfg.app.tools_shortcut_enabled, tools_shortcut = %cfg.app.tools_shortcut, "设置已保存");
    *s.config.lock().unwrap() = cfg;
    // 重新打开时立即检查一次，后台循环最长要等 6 小时。
    if check_enabled {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = update::check_now(&app, None).await;
        });
    }
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
pub async fn open_log_dir(app: AppHandle, s: S<'_>) -> ApiResult<()> {
    let dir = s.log_dir();
    let _ = std::fs::create_dir_all(&dir);
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

/// 弹出保存对话框，把 PNG 写到用户选择的位置；返回保存路径，取消时为空。
#[tauri::command]
pub async fn tools_save_png(app: AppHandle, name: String, data: Vec<u8>) -> ApiResult<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut dlg = app.dialog().file().set_file_name(&name).add_filter("PNG", &["png"]);
    if let Some(dir) = dirs::download_dir().or_else(dirs::home_dir) {
        dlg = dlg.set_directory(dir);
    }
    dlg.save_file(move |p| {
        let _ = tx.send(p);
    });
    let Some(path) = rx.await.ok().flatten().and_then(|p| p.into_path().ok()) else {
        return Ok(String::new());
    };
    std::fs::write(&path, data).map_err(|e| error::FILE_SAVE_FAILED.cause(e))?;
    info!(path = %path.display(), "已保存图片");
    Ok(path.display().to_string())
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

#[tauri::command]
pub async fn update_status(s: S<'_>) -> ApiResult<UpdateInfo> {
    Ok(s.updater.info())
}

#[tauri::command]
pub async fn update_check(app: AppHandle, via: Option<String>) -> ApiResult<UpdateInfo> {
    update::check_now(&app, update::proxy(&app, via.as_deref())?).await
}

/// 是否有待打开的更新页面请求（来自托盘），取出后清除。
#[tauri::command]
pub async fn update_take_open_request(s: S<'_>) -> ApiResult<bool> {
    Ok(s.updater.take_open_request())
}

/// 是否有待打开的工具页请求（来自快捷键），取出后清除。
#[tauri::command]
pub async fn tools_take_open_request() -> ApiResult<bool> {
    Ok(shortcut::take_open_request())
}

/// 设置页录制快捷键时暂时注销，避免按下当前快捷键时被系统拦截；结束后按设置重新注册。
#[tauri::command]
pub async fn tools_shortcut_suspend(app: AppHandle, s: S<'_>, suspend: bool) -> ApiResult<SettingsView> {
    let (enabled, accel) = {
        let cfg = s.config.lock().unwrap();
        (cfg.app.tools_shortcut_enabled && !suspend, cfg.app.tools_shortcut.clone())
    };
    let _ = shortcut::apply(&app, enabled, &accel);
    Ok(settings_view(&app, &s))
}

#[tauri::command]
pub async fn update_install(app: AppHandle, via: Option<String>) -> ApiResult<()> {
    update::install(&app, update::proxy(&app, via.as_deref())?).await
}

#[tauri::command]
pub async fn open_release_page(app: AppHandle) -> ApiResult<()> {
    update::open_release_page(&app)
}
