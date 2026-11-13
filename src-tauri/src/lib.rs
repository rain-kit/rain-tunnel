#[macro_use]
pub mod i18n;
pub mod app;
pub mod commands;
pub mod config;
pub mod core;
pub mod error;
pub mod events;
pub mod logging;
pub mod model;
pub mod secrets;
pub mod service;
pub mod store;
pub mod traffic;
pub mod tray;
pub mod util;
pub mod window;

use tauri_plugin_autostart::MacosLauncher;
use tracing::{info, warn};

pub fn run() {
    let data_dir = config::default_data_dir();
    let _ = std::fs::create_dir_all(&data_dir);
    let (cfg, cfg_err) = match config::load(&data_dir) {
        Ok(c) => (c, None),
        Err(e) => (config::AppConfig::default(), Some(e)),
    };
    i18n::set(&cfg.ui.language);
    let level = logging::init(&data_dir.join("logs"), &cfg.system.log_level);
    if let Some(e) = cfg_err {
        warn!(err = %e, "读取配置失败，使用默认配置");
    }
    info!(version = env!("CARGO_PKG_VERSION"), data_dir = %data_dir.display(), "程序启动");

    tauri::Builder::default()
        // 必须最先注册：重复启动时把已运行实例的窗口显示出来。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| window::show_main(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::tunnel_list,
            commands::tunnel_get,
            commands::tunnel_save,
            commands::tunnel_duplicate,
            commands::tunnel_delete,
            commands::tunnel_reorder,
            commands::tunnel_start,
            commands::tunnel_stop,
            commands::tunnel_start_all,
            commands::tunnel_stop_all,
            commands::hostkey_pending,
            commands::hostkey_answer,
            commands::conn_list,
            commands::conn_close,
            commands::conn_close_all,
            commands::traffic_current,
            commands::traffic_history,
            commands::settings_get,
            commands::settings_save,
            commands::settings_set_autolaunch,
            commands::open_data_dir,
            commands::open_log_dir,
            commands::pick_key_file,
            commands::logs_list,
            commands::logs_clear,
        ])
        .setup(move |app| app::setup(app, data_dir, cfg, level))
        .build(tauri::generate_context!())
        .expect("启动失败")
        .run(app::on_run_event);
}
