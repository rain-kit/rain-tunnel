use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{App, AppHandle, Emitter, Manager, RunEvent};
use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::core::ssh::known_hosts::HostKeyStore;
use crate::events::{self, EventSink};
use crate::logging::{self, LevelHandle};
use crate::secrets::{KeyringBackend, Secrets};
use crate::service::{TunnelService, Tuning};
use crate::store::TunnelStore;
use crate::traffic::TrafficService;
use crate::update::{self, Updater};
use crate::usage::UsageStore;
use crate::{tray, window};

/// 每日流量写盘间隔（秒），退出时也会写一次。
const USAGE_SAVE_SECS: u64 = 60;

pub struct AppState {
    pub data_dir: PathBuf,
    pub config: Mutex<AppConfig>,
    pub tunnels: Arc<TunnelService>,
    pub traffic: TrafficService,
    pub level: LevelHandle,
    pub updater: Updater,
}

impl AppState {
    pub fn log_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }
}

struct TauriSink(AppHandle);

impl EventSink for TauriSink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        let _ = self.0.emit(event, payload);
    }

    fn show_main_window(&self) {
        let app = self.0.clone();
        let _ = self.0.run_on_main_thread(move || window::show_main(&app));
    }
}

pub fn setup(app: &mut App, data_dir: PathBuf, cfg: AppConfig, level: LevelHandle) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();

    let host_keys = Arc::new(HostKeyStore::new(data_dir.join("known_hosts"), HostKeyStore::user_known_hosts()));
    let tunnels = TunnelService::new(
        TunnelStore::new(data_dir.join("tunnels.json")),
        Secrets::new(Box::new(KeyringBackend)),
        host_keys,
        Arc::new(TauriSink(handle.clone())),
        Tuning::default(),
    );
    let silent = cfg.app.silent_start;
    let usage = UsageStore::open(data_dir.join("traffic.json"));
    app.manage(AppState {
        data_dir,
        config: Mutex::new(cfg),
        tunnels: tunnels.clone(),
        traffic: TrafficService::new(usage),
        level,
        updater: Updater::new(app.package_info().version.to_string()),
    });

    let log_app = handle.clone();
    logging::ring().set_listener(move |e| {
        let _ = log_app.emit(events::LOG, e);
    });

    if let Err(e) = tray::create(&handle) {
        error!(err = %e, "创建托盘图标失败");
    }
    let tray_app = handle.clone();
    tunnels.on_change(move || tray::refresh_soon(&tray_app));

    if silent {
        window::set_dock_visible(&handle, false);
    } else {
        window::show_main(&handle);
    }

    let traffic_app = handle.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        for n in 1u64.. {
            tick.tick().await;
            let st = traffic_app.state::<AppState>();
            let tr = st.traffic.sample(&st.tunnels.snapshot_running());
            let _ = traffic_app.emit(events::TRAFFIC, tr);
            if n % USAGE_SAVE_SECS == 0 {
                st.traffic.save_usage();
            }
        }
    });

    update::spawn(handle.clone());

    tauri::async_runtime::spawn(async move {
        let failed = tunnels.start_auto().await;
        for (name, err) in failed {
            warn!(tunnel = %name, err = err.text(), "自动启动隧道失败");
        }
    });
    Ok(())
}

pub fn on_run_event(app: &AppHandle, ev: RunEvent) {
    match ev {
        // 最后一个窗口关闭时：开启“关闭到托盘”则保留后台运行，窗口已销毁以释放内存。
        RunEvent::ExitRequested { code: None, api, .. } => {
            let to_tray = app.state::<AppState>().config.lock().unwrap().app.close_to_tray;
            if to_tray {
                api.prevent_exit();
                window::set_dock_visible(app, false);
            } else {
                app.exit(0);
            }
        }
        RunEvent::Exit => {
            let tunnels = app.state::<AppState>().tunnels.clone();
            tauri::async_runtime::block_on(async move {
                if tokio::time::timeout(Duration::from_secs(10), tunnels.stop_all()).await.is_err() {
                    warn!("停止隧道超时，直接退出");
                }
            });
            app.state::<AppState>().traffic.flush();
            info!("程序退出");
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => window::show_main(app),
        _ => {}
    }
}
