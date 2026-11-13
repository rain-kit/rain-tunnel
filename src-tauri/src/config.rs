use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::i18n::{LANGUAGES, Msg};
use crate::util::write_file_atomic;

pub const APP_NAME: &str = "Rain Tunnel";
/// 数据目录名与钥匙串前缀沿用 v1，改动会导致已有配置和已保存的密码无法读取。
pub const DATA_DIR_NAME: &str = "ssh-tunnel";
pub const KEYRING_PREFIX: &str = "ssh-tunnel";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SystemConfig {
    /// 日志级别 debug/info/warn/error
    pub log_level: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        SystemConfig { log_level: "info".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiConfig {
    /// 主题 system/light/dark
    pub theme: String,
    /// 界面语言 system/zh-CN/zh-TW/en
    pub language: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        UiConfig { theme: "system".into(), language: "system".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSection {
    /// 关闭窗口时保留在托盘（销毁界面以节省内存）
    pub close_to_tray: bool,
    /// 启动时不显示主窗口
    pub silent_start: bool,
}

impl Default for AppSection {
    fn default() -> Self {
        AppSection { close_to_tray: true, silent_start: false }
    }
}

/// config.yaml 的内容，与 v1 的格式一致。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub system: SystemConfig,
    pub ui: UiConfig,
    pub app: AppSection,
}

pub const LOG_LEVELS: [&str; 4] = ["debug", "info", "warn", "error"];
pub const THEMES: [&str; 3] = ["system", "light", "dark"];

impl AppConfig {
    pub fn validate(&self) -> Result<(), Msg> {
        if !LOG_LEVELS.contains(&self.system.log_level.as_str()) {
            return Err(msg!("err.config.logLevelInvalid", "日志级别无效: {value}", value = self.system.log_level));
        }
        if !THEMES.contains(&self.ui.theme.as_str()) {
            return Err(msg!("err.config.themeInvalid", "主题无效: {value}", value = self.ui.theme));
        }
        if !LANGUAGES.contains(&self.ui.language.as_str()) {
            return Err(msg!("err.config.languageInvalid", "语言无效: {value}", value = self.ui.language));
        }
        Ok(())
    }

    /// 环境变量可以覆盖配置：把 yaml 路径里的 . 换成 _ 并大写，例如 SYSTEM_LOGLEVEL=debug。
    fn apply_env(&mut self) {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let flag = |v: String| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on");
        if let Some(v) = var("SYSTEM_LOGLEVEL") {
            self.system.log_level = v.to_ascii_lowercase();
        }
        if let Some(v) = var("UI_THEME") {
            self.ui.theme = v.to_ascii_lowercase();
        }
        if let Some(v) = var("UI_LANGUAGE") {
            self.ui.language = v;
        }
        if let Some(v) = var("APP_CLOSETOTRAY") {
            self.app.close_to_tray = flag(v);
        }
        if let Some(v) = var("APP_SILENTSTART") {
            self.app.silent_start = flag(v);
        }
    }
}

pub fn config_file(data_dir: &Path) -> PathBuf {
    data_dir.join("config.yaml")
}

/// 读取数据目录下的 config.yaml，不存在时写入默认配置。
pub fn load(data_dir: &Path) -> Result<AppConfig, String> {
    let path = config_file(data_dir);
    let mut cfg = match std::fs::read_to_string(&path) {
        Ok(text) if text.trim().is_empty() => AppConfig::default(),
        Ok(text) => serde_yaml::from_str(&text).map_err(|e| format!("解析 {} 失败: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let cfg = AppConfig::default();
            save(data_dir, &cfg).map_err(|e| format!("写入默认配置失败: {e}"))?;
            cfg
        }
        Err(e) => return Err(format!("读取 {} 失败: {e}", path.display())),
    };
    cfg.apply_env();
    if !LOG_LEVELS.contains(&cfg.system.log_level.as_str()) {
        cfg.system.log_level = "info".into();
    }
    if !THEMES.contains(&cfg.ui.theme.as_str()) {
        cfg.ui.theme = "system".into();
    }
    if !LANGUAGES.contains(&cfg.ui.language.as_str()) {
        cfg.ui.language = "system".into();
    }
    Ok(cfg)
}

pub fn save(data_dir: &Path, cfg: &AppConfig) -> std::io::Result<()> {
    let text = serde_yaml::to_string(cfg).map_err(std::io::Error::other)?;
    write_file_atomic(&config_file(data_dir), text.as_bytes())
}

/// 数据目录：环境变量 SSH_TUNNEL_DATA_DIR 优先（便携模式、测试用），
/// 否则为 Windows 的 %AppData%\ssh-tunnel 或 macOS 的 ~/Library/Application Support/ssh-tunnel，与 v1 相同。
pub fn default_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SSH_TUNNEL_DATA_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join(DATA_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_compatible_with_v1() {
        let v1 = "system:\n    logLevel: debug\nui:\n    theme: dark\napp:\n    silentStart: true\n";
        let cfg: AppConfig = serde_yaml::from_str(v1).unwrap();
        assert_eq!(cfg.system.log_level, "debug");
        assert_eq!(cfg.ui.theme, "dark");
        assert!(cfg.app.silent_start);
        assert!(cfg.app.close_to_tray, "缺省字段保持默认值");
    }

    #[test]
    fn load_writes_default() {
        let dir = std::env::temp_dir().join(format!("st-cfg-{}", crate::util::new_id()));
        let cfg = load(&dir).unwrap();
        assert_eq!(cfg, AppConfig::default());
        assert!(config_file(&dir).exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
