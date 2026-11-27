//! 打开工具页的全局快捷键。快捷键写法见 global-hotkey，例如 "Alt+Space"、"Ctrl+Shift+KeyT"。

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Emitter, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tracing::{info, warn};

use crate::error::{self, ApiError};
use crate::i18n::Msg;
use crate::{events, window};

/// 按下快捷键时窗口可能刚创建、还没订阅事件，所以由界面加载后来取。
static OPEN_REQUESTED: AtomicBool = AtomicBool::new(false);
/// 当前快捷键注册失败的原因，设置页显示。
static LAST_ERROR: Mutex<Option<Msg>> = Mutex::new(None);

/// macOS 上 Alt 即 Option；Command+Space 被 Spotlight 占用，不能做默认值。
pub fn default_shortcut() -> &'static str {
    "Alt+Space"
}

/// v1.0.8、v1.0.9 在 macOS 上的默认快捷键，读取配置时换成新的默认值。
pub const OLD_MAC_DEFAULT: &str = "Cmd+Space";

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                open_tools(app);
            }
        })
        .build()
}

/// 显示主窗口并切到工具页。
pub fn open_tools(app: &AppHandle) {
    OPEN_REQUESTED.store(true, Ordering::SeqCst);
    window::show_main(app);
    let _ = app.emit(events::TOOLS_OPEN, ());
}

pub fn take_open_request() -> bool {
    OPEN_REQUESTED.swap(false, Ordering::SeqCst)
}

pub fn last_error() -> Option<Msg> {
    LAST_ERROR.lock().unwrap().clone()
}

/// 按设置注册快捷键，替换之前注册的；enabled 为 false 时只注销。
pub fn apply(app: &AppHandle, enabled: bool, accel: &str) -> Result<(), ApiError> {
    let _ = app.global_shortcut().unregister_all();
    let r = if enabled { register(app, accel) } else { Ok(()) };
    *LAST_ERROR.lock().unwrap() = r.as_ref().err().map(|e| e.msg.clone());
    r
}

fn register(app: &AppHandle, accel: &str) -> Result<(), ApiError> {
    let shortcut: Shortcut = accel
        .parse()
        .map_err(|e| error::SHORTCUT_INVALID.msg(msg!("err.shortcutInvalid", "快捷键无效: {shortcut}", shortcut = accel).with_cause(e)))?;
    app.global_shortcut().register(shortcut).map_err(|e| {
        warn!(shortcut = accel, err = %e, "注册工具快捷键失败");
        error::SHORTCUT_REGISTER_FAILED
            .msg(msg!("err.shortcutRegisterFailed", "注册快捷键 {shortcut} 失败，可能已被系统或其他程序占用", shortcut = accel))
    })?;
    info!(shortcut = accel, "已注册工具快捷键");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontend_accelerators() {
        for accel in ["Alt+Space", "Cmd+Space", "Ctrl+Shift+KeyT", "Super+Digit1", "Ctrl+Alt+BracketLeft", "F5", default_shortcut()] {
            assert!(accel.parse::<Shortcut>().is_ok(), "{accel}");
        }
        for accel in ["", "Ctrl+", "Ctrl+Foo", "KeyA+Ctrl"] {
            assert!(accel.parse::<Shortcut>().is_err(), "{accel}");
        }
    }
}
