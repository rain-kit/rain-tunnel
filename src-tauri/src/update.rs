//! 版本检测与更新：后台每 6 小时查询 GitHub 上最新的正式版（不含预发布，可在设置中关闭），用户点击更新时下载并安装。

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine;
use chrono::{DateTime, Local, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;
use tracing::{info, warn};

use crate::app::AppState;
use crate::error::{self, ApiError, ApiResult};
use crate::model::{TunnelState, TunnelType};
use crate::{events, tray, window};

pub const RELEASES_URL: &str = "https://github.com/rain-kit/rain-tunnel/releases/latest";
const LATEST_API: &str = "https://api.github.com/repos/rain-kit/rain-tunnel/releases/latest";
const INTERVAL: Duration = Duration::from_secs(6 * 3600);
/// 启动后稍等再检查，避开隧道自动启动的高峰。
const FIRST_DELAY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    /// 尚未成功检查过时为空
    pub latest: Option<String>,
    pub has_update: bool,
    pub checked_at: Option<DateTime<Local>>,
    /// 最新版本的更新内容（Markdown），取自 Release 说明中的更新内容一节
    pub notes: Option<String>,
    pub published_at: Option<DateTime<Local>>,
    /// 新版本能否直接更新：要有当前平台的安装包，且签名出自内置公钥，否则只能前往下载页。
    pub can_install: bool,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

pub struct Updater {
    info: Mutex<UpdateInfo>,
    /// 托盘请求打开更新页面；窗口可能刚创建、还没订阅事件，所以由界面加载后来取。
    open_requested: AtomicBool,
    /// 下载后验签失败的签名。预检看不出这种错误，记下来避免每次检查后又显示直接更新；
    /// 新版本或重新上传的安装包签名不同，会重新判断。
    rejected_signature: Mutex<Option<String>>,
}

impl Updater {
    pub fn new(current: String) -> Self {
        Updater {
            info: Mutex::new(UpdateInfo { current, ..Default::default() }),
            open_requested: AtomicBool::new(false),
            rejected_signature: Mutex::new(None),
        }
    }

    pub fn info(&self) -> UpdateInfo {
        self.info.lock().unwrap().clone()
    }

    pub fn take_open_request(&self) -> bool {
        self.open_requested.swap(false, Ordering::SeqCst)
    }

    async fn fetch_latest(&self, proxy: Option<&Url>) -> Result<Release, reqwest::Error> {
        let current = self.info.lock().unwrap().current.clone();
        let mut client = reqwest::Client::builder()
            // GitHub API 要求带 User-Agent。
            .user_agent(format!("rain-tunnel/{current}"))
            .timeout(Duration::from_secs(15));
        if let Some(p) = proxy {
            client = client.proxy(reqwest::Proxy::all(p.as_str())?);
        }
        client
            .build()?
            .get(LATEST_API)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    async fn check(&self, proxy: Option<&Url>) -> ApiResult<UpdateInfo> {
        let rel = self.fetch_latest(proxy).await.map_err(|e| error::UPDATE_CHECK_FAILED.cause(e))?;
        let latest = rel.tag_name.trim_start_matches('v').to_string();
        let mut info = self.info.lock().unwrap();
        info.has_update = is_newer(&latest, &info.current);
        // 新版本先当作能直接更新，由预检纠正；预检连不上时安装前还会再验一次。
        if !info.has_update {
            info.can_install = false;
        } else if info.latest.as_deref() != Some(latest.as_str()) {
            info.can_install = true;
        }
        info.latest = Some(latest);
        info.notes = rel.body.as_deref().and_then(extract_notes);
        info.published_at = rel.published_at.map(|t| t.with_timezone(&Local));
        info.checked_at = Some(Local::now());
        Ok(info.clone())
    }

    fn set_can_install(&self, can: bool) -> UpdateInfo {
        let mut info = self.info.lock().unwrap();
        info.can_install = can && info.has_update;
        info.clone()
    }
}

fn pubkey(app: &AppHandle) -> String {
    app.config().plugins.0.get("updater").and_then(|c| c["pubkey"].as_str()).unwrap_or_default().to_string()
}

/// 检查、更新时选择的访问方式：via 为动态转发隧道 ID 时经它的 SOCKS5 代理，为空时直接访问。
pub fn proxy(app: &AppHandle, via: Option<&str>) -> ApiResult<Option<Url>> {
    let Some(id) = via.filter(|id| !id.is_empty()) else { return Ok(None) };
    let runner = app.state::<AppState>().tunnels.snapshot_running().remove(id);
    runner
        .filter(|r| r.def().kind == TunnelType::Dynamic && r.status().state == TunnelState::Connected)
        .and_then(|r| proxy_url(&r.listen_addr(), &r.def().proxy_user, &r.def().proxy_pass))
        .map(Some)
        .ok_or_else(|| error::UPDATE_TUNNEL_UNAVAILABLE.into())
}

/// 用 socks5h 让 SSH 服务器解析域名；监听所有地址时经本机回环连接。
fn proxy_url(listen: &str, user: &str, pass: &str) -> Option<Url> {
    let addr = match (listen.strip_prefix("0.0.0.0:"), listen.strip_prefix("[::]:")) {
        (Some(port), _) => format!("127.0.0.1:{port}"),
        (_, Some(port)) => format!("[::1]:{port}"),
        _ => listen.to_string(),
    };
    let mut url = Url::parse(&format!("socks5h://{addr}")).ok()?;
    if !user.is_empty() {
        url.set_username(user).ok()?;
        url.set_password(Some(pass)).ok()?;
    }
    Some(url)
}

/// 下载安装包可能很久，只给检查设置超时。
fn updater(app: &AppHandle, proxy: Option<Url>, timeout: Option<Duration>) -> tauri_plugin_updater::Result<tauri_plugin_updater::Updater> {
    let mut b = app.updater_builder();
    if let Some(t) = timeout {
        b = b.timeout(t);
    }
    if let Some(p) = proxy {
        b = b.proxy(p);
    }
    b.build()
}

/// 不下载安装包，先用 latest.json 预检能否直接更新。完整验签要等下载完才能做，失败时由 install 关掉直接更新。
/// 连不上时返回 None，不当作签名错误，保留之前的判断。
async fn precheck(app: &AppHandle, latest: &str, proxy: Option<Url>) -> Option<bool> {
    use tauri_plugin_updater::Error::{Network, Reqwest};
    let reason = match async { updater(app, proxy, Some(Duration::from_secs(15)))?.check().await }.await {
        Err(e @ (Reqwest(_) | Network(_))) => {
            warn!(latest, err = %e, "获取 latest.json 失败，暂不判断能否直接更新");
            return None;
        }
        Ok(Some(u)) if u.version.trim_start_matches('v') != latest => format!("latest.json 的版本 {} 与最新版本不符", u.version),
        Ok(Some(u)) if app.state::<AppState>().updater.rejected_signature.lock().unwrap().as_ref() == Some(&u.signature) => {
            "安装包与签名不符".to_string()
        }
        Ok(Some(u)) => match check_signature(&u.signature, &pubkey(app), latest) {
            Ok(()) => return Some(true),
            Err(reason) => reason.to_string(),
        },
        Ok(None) => "latest.json 中没有比当前更新的版本".to_string(),
        Err(e) => e.to_string(),
    };
    warn!(latest, %reason, "新版本不能直接更新，只提供下载页");
    Some(false)
}

/// 核对签名所用的密钥和签名时记录的版本；签名本身要配合安装包才能校验。
/// 公钥和签名都是 base64 编码的 minisign 文本，第二行解码后依次是 2 字节算法、8 字节密钥 ID、公钥或签名。
fn check_signature(signature: &str, pubkey: &str, version: &str) -> Result<(), &'static str> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let decode = |s: &str, len: usize| -> Option<(Vec<u8>, String)> {
        let text = String::from_utf8(b64.decode(s.trim()).ok()?).ok()?;
        let bin = b64.decode(text.lines().nth(1)?.trim()).ok()?;
        (bin.len() == len).then_some((bin, text))
    };
    let (key, _) = decode(pubkey, 42).ok_or("内置公钥无效")?;
    let (sig, text) = decode(signature, 74).ok_or("签名格式无效")?;
    if key[2..10] != sig[2..10] {
        return Err("签名与内置公钥不匹配");
    }
    let signed = text
        .lines()
        .find_map(|l| l.strip_prefix("trusted comment: "))
        .and_then(|c| c.split('\t').find_map(|f| f.strip_prefix("version:")));
    match signed {
        Some(v) if v.trim_start_matches('v') != version.trim_start_matches('v') => Err("签名的版本与最新版本不符"),
        _ => Ok(()),
    }
}

fn disable_install(app: &AppHandle) {
    let info = app.state::<AppState>().updater.set_can_install(false);
    let _ = app.emit(events::UPDATE, &info);
}

fn is_signature_error(e: &tauri_plugin_updater::Error) -> bool {
    use tauri_plugin_updater::Error::*;
    matches!(e, Minisign(_) | Base64(_) | SignatureUtf8(_) | SignedVersionMismatch { .. } | MissingSignedVersion)
}

/// 取 Release 说明中 changes:start 与 changes:end 标记之间的内容（见 .github/release-notes.md）。
fn extract_notes(body: &str) -> Option<String> {
    let start = body.find("<!-- changes:start -->")? + "<!-- changes:start -->".len();
    let len = body[start..].find("<!-- changes:end -->")?;
    let notes = body[start..start + len].trim();
    (!notes.is_empty()).then(|| notes.replace("\r\n", "\n"))
}

/// 托盘菜单“发现新版本”：显示主窗口并打开更新页面。
pub fn request_open(app: &AppHandle) {
    app.state::<AppState>().updater.open_requested.store(true, Ordering::SeqCst);
    window::show_main(app);
    let _ = app.emit(events::UPDATE_OPEN, ());
}

/// 检查一次，把结果推送给界面并刷新托盘菜单。proxy 为空时直接访问。
pub async fn check_now(app: &AppHandle, proxy: Option<Url>) -> Result<UpdateInfo, ApiError> {
    let st = app.state::<AppState>();
    let before = st.updater.info().latest;
    match st.updater.check(proxy.as_ref()).await {
        Ok(mut info) => {
            if info.has_update
                && let Some(can) = precheck(app, info.latest.as_deref().unwrap_or_default(), proxy).await
            {
                info = st.updater.set_can_install(can);
            }
            if info.has_update && before != info.latest {
                info!(current = %info.current, latest = info.latest.as_deref().unwrap_or_default(), "发现新版本");
            }
            let _ = app.emit(events::UPDATE, &info);
            tray::refresh_soon(app);
            Ok(info)
        }
        Err(e) => {
            warn!(err = %e, "检查更新失败");
            Err(e)
        }
    }
}

/// 下载并安装新版本（安装包来自 release 中的 latest.json，经 tauri.conf.json 里的公钥验签）。
/// 下载完成后先停掉隧道再安装；Windows 由安装程序退出并重新启动应用，其他系统安装后重启。
pub async fn install(app: &AppHandle, proxy: Option<Url>) -> ApiResult<()> {
    let fail = |e: tauri_plugin_updater::Error| error::UPDATE_INSTALL_FAILED.cause(e);
    let update = updater(app, proxy, None).map_err(fail)?.check().await.map_err(fail)?;
    let Some(update) = update.filter(|u| check_signature(&u.signature, &pubkey(app), &u.version).is_ok()) else {
        disable_install(app);
        return Err(error::UPDATE_NOT_AVAILABLE.into());
    };
    info!(version = %update.version, "开始下载更新");
    let emitter = app.clone();
    let (mut downloaded, mut reported) = (0u64, 0u64);
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                // 分块很碎，每 256KB 或下载完成时才通知一次。
                if downloaded - reported >= 256 * 1024 || Some(downloaded) == total {
                    reported = downloaded;
                    let _ = emitter.emit(events::UPDATE_PROGRESS, Progress { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| {
            if is_signature_error(&e) {
                *app.state::<AppState>().updater.rejected_signature.lock().unwrap() = Some(update.signature.clone());
                disable_install(app);
            }
            fail(e)
        })?;

    let tunnels = app.state::<AppState>().tunnels.clone();
    let running: Vec<String> =
        tunnels.list_status().into_iter().filter(|(_, st)| st.state != TunnelState::Stopped).map(|(t, _)| t.id).collect();
    if tokio::time::timeout(Duration::from_secs(10), tunnels.stop_all()).await.is_err() {
        warn!("停止隧道超时，继续安装更新");
    }
    info!(version = %update.version, "安装更新");
    if let Err(e) = update.install(&bytes) {
        for id in &running {
            let _ = tunnels.start(id).await;
        }
        return Err(fail(e));
    }
    app.restart();
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_DELAY).await;
        loop {
            let enabled = app.state::<AppState>().config.lock().unwrap().app.auto_check_update;
            if enabled {
                let _ = check_now(&app, None).await;
            }
            tokio::time::sleep(INTERVAL).await;
        }
    });
}

pub fn open_release_page(app: &AppHandle) -> ApiResult<()> {
    app.opener().open_url(RELEASES_URL, None::<&str>).map_err(|e| error::OPEN_URL_FAILED.cause(e))
}

/// 按数字段比较版本号，忽略 -beta 之类的后缀。
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split(['-', '+']).next().unwrap_or_default().split('.').map(|p| p.parse().unwrap_or(0)).collect()
    };
    let (a, b) = (parse(latest), parse(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use base64::Engine;

    use super::{check_signature, extract_notes, is_newer, proxy_url};

    // tauri-plugin-updater 测试用的密钥对和签名，签名记录的版本是 2.0.0。
    const PUBLIC_KEY: &str = "untrusted comment: minisign public key: A49EF4746FA6E6D7\nRWTX5qZvdPSepDGWg2bz8n6ArQnBMwgQK1geVosorcbLzcunD+KJ+cYe";
    const OTHER_PUBLIC_KEY: &str = "untrusted comment: minisign public key E7620F1842B4E81F\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const SIGNATURE: &str = "untrusted comment: signature from tauri secret key\nRUTX5qZvdPSepPF2zsjvP/L/TQtBLRnWRvaq+4omK1UEugtQp/USrGiI1v762sDy3id3aor7bRC9ETyeot6lMHHrWoZ5hjUM1Qc=\ntrusted comment: timestamp:1791649051\tfile:payload\tversion:2.0.0\nDDq2CLOEl5cQmC2eJRHlnZPVViOZsu1CjpPKQoei4A/XBfkDqGO0B+QMrcrHgEQ5qV3mne75tdFWhJpqUXh9AA==";

    #[test]
    fn proxy_urls() {
        let url = |l: &str, u: &str, p: &str| proxy_url(l, u, p).map(|u| u.to_string());
        assert_eq!(url("127.0.0.1:1080", "", "").as_deref(), Some("socks5h://127.0.0.1:1080"));
        assert_eq!(url("0.0.0.0:1080", "", "").as_deref(), Some("socks5h://127.0.0.1:1080"));
        assert_eq!(url("[::]:1080", "", "").as_deref(), Some("socks5h://[::1]:1080"));
        assert_eq!(url("[::1]:1080", "", "").as_deref(), Some("socks5h://[::1]:1080"));
        assert_eq!(url("127.0.0.1:1080", "me", "p@ss:w").as_deref(), Some("socks5h://me:p%40ss%3Aw@127.0.0.1:1080"));
    }

    #[test]
    fn signature_precheck() {
        let enc = |s: &str| base64::engine::general_purpose::STANDARD.encode(s);
        assert_eq!(check_signature(&enc(SIGNATURE), &enc(PUBLIC_KEY), "2.0.0"), Ok(()));
        assert_eq!(check_signature(&enc(SIGNATURE), &enc(PUBLIC_KEY), "v2.0.0"), Ok(()));
        assert!(check_signature(&enc(SIGNATURE), &enc(OTHER_PUBLIC_KEY), "2.0.0").is_err());
        assert!(check_signature(&enc(SIGNATURE), &enc(PUBLIC_KEY), "2.0.1").is_err());
        assert!(check_signature("invalid signature", &enc(PUBLIC_KEY), "2.0.0").is_err());
        assert!(check_signature(&enc(SIGNATURE), "", "2.0.0").is_err());
    }

    #[test]
    fn notes_between_markers() {
        let body = "## 更新内容\r\n\r\n<!-- changes:start -->\r\n- a\r\n- b\r\n<!-- changes:end -->\r\n\r\n## 下载\r\n| x |";
        assert_eq!(extract_notes(body).as_deref(), Some("- a\n- b"));
        assert_eq!(extract_notes("## 下载\n| x |"), None);
        assert_eq!(extract_notes("<!-- changes:start -->\n\n<!-- changes:end -->"), None);
    }

    #[test]
    fn compare_versions() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("2.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(!is_newer("0.9.9", "1.0.0"));
        assert!(!is_newer("1.0.0-beta", "1.0.0"));
    }
}
