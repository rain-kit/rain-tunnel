use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tracing::{error, warn};

use crate::app::AppState;
use crate::i18n::pick;
use crate::model::TunnelState;
use crate::window;

const TRAY_ID: &str = "main";
const TUNNEL_PREFIX: &str = "tunnel:";

static REFRESH_PENDING: AtomicBool = AtomicBool::new(false);

fn icon() -> Image<'static> {
    #[cfg(target_os = "macos")]
    let bytes = include_bytes!("../icons/tray-template.png");
    #[cfg(not(target_os = "macos"))]
    let bytes = include_bytes!("../icons/tray.png");
    Image::from_bytes(bytes).expect("托盘图标无效")
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let (menu, tooltip) = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon())
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip(tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| on_menu(app, ev.id().as_ref()))
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                window::show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// 隧道状态频繁变化时合并刷新，300ms 内最多重建一次菜单。
pub fn refresh_soon(app: &AppHandle) {
    if REFRESH_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        REFRESH_PENDING.store(false, Ordering::SeqCst);
        let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
        match build_menu(&app) {
            Ok((menu, tooltip)) => {
                let _ = tray.set_menu(Some(menu));
                let _ = tray.set_tooltip(Some(tooltip));
            }
            Err(e) => error!(err = %e, "刷新托盘菜单失败"),
        }
    });
}

fn build_menu(app: &AppHandle) -> tauri::Result<(Menu<Wry>, String)> {
    let state = app.state::<AppState>();
    let list = state.tunnels.list_status();
    let (running, connected) = state.tunnels.running_count();

    let mut tunnel_items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    if list.is_empty() {
        let text = pick(["暂无隧道", "尚無隧道", "No tunnels"]);
        tunnel_items.push(Box::new(MenuItem::with_id(app, "none", text, false, None::<&str>)?));
    }
    for (t, st) in &list {
        let (name, label) = (&t.name, st.state.label());
        let text = tr!("{name}（{label}）", "{name}（{label}）", "{name} ({label})");
        let checked = st.state != TunnelState::Stopped;
        let id = format!("{TUNNEL_PREFIX}{}", t.id);
        tunnel_items.push(Box::new(CheckMenuItem::with_id(app, id, text, true, checked, None::<&str>)?));
    }

    let show = MenuItem::with_id(app, "show", pick(["显示主界面", "顯示主視窗", "Show Window"]), true, None::<&str>)?;
    let start_all = MenuItem::with_id(app, "start_all", pick(["全部启动", "全部啟動", "Start All"]), !list.is_empty(), None::<&str>)?;
    let stop_all = MenuItem::with_id(app, "stop_all", pick(["全部停止", "全部停止", "Stop All"]), running > 0, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", pick(["退出", "結束", "Quit"]), true, None::<&str>)?;

    let mut items: Vec<&dyn IsMenuItem<Wry>> = vec![&show];
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    items.push(&sep1);
    items.extend(tunnel_items.iter().map(|b| b.as_ref()));
    items.push(&sep2);
    items.push(&start_all);
    items.push(&stop_all);
    items.push(&sep3);
    items.push(&quit);
    let menu = Menu::with_items(app, &items)?;

    let tooltip = tr!(
        "Rain Tunnel\n运行 {running} 条，已连接 {connected} 条",
        "Rain Tunnel\n執行 {running} 條，已連線 {connected} 條",
        "Rain Tunnel\n{running} running, {connected} connected"
    );
    Ok((menu, tooltip))
}

fn on_menu(app: &AppHandle, id: &str) {
    let svc = app.state::<AppState>().tunnels.clone();
    match id {
        "show" => window::show_main(app),
        "quit" => app.exit(0),
        "start_all" => {
            tauri::async_runtime::spawn(async move {
                for (name, err) in svc.start_all().await {
                    warn!(tunnel = %name, err = err.text(), "托盘启动隧道失败");
                }
            });
        }
        "stop_all" => {
            tauri::async_runtime::spawn(async move { svc.stop_all().await });
        }
        _ => {
            let Some(tid) = id.strip_prefix(TUNNEL_PREFIX).map(str::to_string) else { return };
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let running = svc.list_status().into_iter().any(|(t, st)| t.id == tid && st.state != TunnelState::Stopped);
                if running {
                    svc.stop(&tid).await;
                } else if let Err(e) = svc.start(&tid).await {
                    warn!(id = %tid, err = %e, "托盘启动隧道失败");
                }
                // 启动失败时状态没变，也要刷新以恢复复选框。
                refresh_soon(&app);
            });
        }
    }
}
