use std::sync::atomic::{AtomicBool, Ordering};

use tauri::window::Color;
use tauri::{AppHandle, Manager, Theme, WebviewUrl, WebviewWindowBuilder};
use tracing::error;

use crate::app::AppState;
use crate::config::APP_NAME;

pub const MAIN: &str = "main";

static CREATING: AtomicBool = AtomicBool::new(false);

pub fn native_theme(theme: &str) -> Option<Theme> {
    match theme {
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None,
    }
}

fn is_dark(app: &AppHandle, theme: Option<Theme>) -> bool {
    match theme {
        Some(t) => t == Theme::Dark,
        None => app
            .get_webview_window(MAIN)
            .and_then(|w| w.theme().ok())
            .map(|t| t == Theme::Dark)
            .unwrap_or_else(system_dark),
    }
}

#[cfg(windows)]
fn system_dark() -> bool {
    use std::process::Command;
    use std::os::windows::process::CommandExt;
    // 读取 AppsUseLightTheme，为 0 表示深色。只在创建窗口时调用一次，用来设置背景色避免白屏闪烁。
    Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "/v", "AppsUseLightTheme"])
        .creation_flags(0x0800_0000)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("0x0"))
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn system_dark() -> bool {
    false
}

/// macOS 上切换 Dock 和 Cmd+Tab 中是否显示应用图标；隐藏后只能从托盘唤回窗口。
pub fn set_dock_visible(app: &AppHandle, visible: bool) {
    #[cfg(target_os = "macos")]
    {
        let policy = if visible { tauri::ActivationPolicy::Regular } else { tauri::ActivationPolicy::Accessory };
        if let Err(e) = app.set_activation_policy(policy) {
            error!(err = %e, "切换 Dock 图标显示失败");
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, visible);
}

/// 显示主窗口；关闭到托盘后窗口已销毁，此时重新创建。
pub fn show_main(app: &AppHandle) {
    set_dock_visible(app, true);
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    if CREATING.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Err(e) = create_main(app) {
        error!(err = %e, "创建主窗口失败");
    }
    CREATING.store(false, Ordering::SeqCst);
}

fn create_main(app: &AppHandle) -> tauri::Result<()> {
    let theme = native_theme(&app.state::<AppState>().config.lock().unwrap().ui.theme);
    let bg = if is_dark(app, theme) { Color(0x0f, 0x11, 0x15, 0xff) } else { Color(0xf6, 0xf7, 0xf9, 0xff) };

    let builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title(APP_NAME)
        .inner_size(1080.0, 720.0)
        .min_inner_size(880.0, 580.0)
        .center()
        .theme(theme)
        .background_color(bg);
    #[cfg(windows)]
    let builder = builder.decorations(false).shadow(true);
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);

    let w = builder.build()?;
    let _ = w.set_focus();
    Ok(())
}

/// 设置页修改主题后同步原生窗口主题（影响 macOS 标题栏和滚动条等）。
pub fn apply_theme(app: &AppHandle, theme: &str) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.set_theme(native_theme(theme));
    }
}
