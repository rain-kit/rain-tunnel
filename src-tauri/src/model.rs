use std::collections::HashMap;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::i18n::Msg;
use crate::util::{is_loopback, join_host_port};

pub const DEFAULT_SSH_PORT: i32 = 22;
pub const DEFAULT_LISTEN_HOST: &str = "127.0.0.1";
pub const DEFAULT_KEEPALIVE: i32 = 15;
pub const DEFAULT_IDLE_TIMEOUT: i32 = 300;
pub const DEFAULT_MAX_CONNS: i32 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TunnelType {
    /// 本地端口转发（ssh -L）：本机监听端口，经 SSH 服务器连接目标。
    Local,
    /// 远程端口转发（ssh -R）：SSH 服务器监听端口，经本机连接目标。
    Remote,
    /// 动态端口转发（ssh -D）：本机开 SOCKS5/HTTP 代理，经 SSH 服务器访问任意目标。
    #[default]
    Dynamic,
}

impl TunnelType {
    pub fn as_str(self) -> &'static str {
        match self {
            TunnelType::Local => "local",
            TunnelType::Remote => "remote",
            TunnelType::Dynamic => "dynamic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    #[default]
    Password,
    Key,
    Agent,
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// 一条隧道的持久化定义，每条隧道自带 SSH 服务器信息。
/// 密码类字段不写入 tunnels.json，由 secrets 存进系统凭据库。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Tunnel {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: TunnelType,

    pub ssh_host: String,
    pub ssh_port: i32,
    pub ssh_user: String,
    pub auth_type: AuthType,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub key_file: String,

    #[serde(skip_serializing_if = "String::is_empty")]
    pub password: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub key_passphrase: String,

    /// 本地、动态转发时是本机监听地址；远程转发时是 SSH 服务器上的监听地址。
    pub listen_host: String,
    pub listen_port: i32,
    /// 本地转发时由 SSH 服务器去连接；远程转发时由本机去连接；动态转发不用。
    #[serde(skip_serializing_if = "String::is_empty")]
    pub target_host: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub target_port: i32,

    // 以下仅动态转发使用。
    #[serde(skip_serializing_if = "is_false")]
    pub enable_http: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub proxy_user: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub proxy_pass: String,

    pub auto_start: bool,
    pub auto_reconnect: bool,
    #[serde(skip_serializing_if = "is_zero")]
    pub keepalive_seconds: i32,
    #[serde(skip_serializing_if = "is_zero")]
    pub idle_timeout_seconds: i32,
    #[serde(skip_serializing_if = "is_zero")]
    pub max_conns: i32,

    pub created_at: Option<DateTime<Local>>,
    pub updated_at: Option<DateTime<Local>>,
}

fn valid_port(p: i32) -> bool {
    (1..=65535).contains(&p)
}

impl Tunnel {
    /// 填充默认值，不覆盖已设置的字段。
    pub fn normalize(&mut self) {
        for s in [
            &mut self.name,
            &mut self.ssh_host,
            &mut self.ssh_user,
            &mut self.listen_host,
            &mut self.target_host,
        ] {
            let trimmed = s.trim();
            if trimmed.len() != s.len() {
                *s = trimmed.to_string();
            }
        }
        if self.ssh_port == 0 {
            self.ssh_port = DEFAULT_SSH_PORT;
        }
        if self.listen_host.is_empty() {
            self.listen_host = DEFAULT_LISTEN_HOST.to_string();
        }
        if self.keepalive_seconds == 0 {
            self.keepalive_seconds = DEFAULT_KEEPALIVE;
        }
        if self.idle_timeout_seconds == 0 {
            self.idle_timeout_seconds = DEFAULT_IDLE_TIMEOUT;
        }
        if self.max_conns == 0 {
            self.max_conns = DEFAULT_MAX_CONNS;
        }
        if self.name.is_empty() {
            self.name = format!("{}@{}:{}", self.ssh_user, self.ssh_host, self.listen_port);
        }
        if self.kind != TunnelType::Dynamic {
            self.enable_http = false;
            self.proxy_user.clear();
            self.proxy_pass.clear();
        }
    }

    pub fn validate(&self) -> Result<(), Msg> {
        if self.ssh_host.is_empty() {
            return Err(msg!("err.tunnel.sshHostRequired", "SSH 服务器地址不能为空"));
        }
        if self.ssh_user.is_empty() {
            return Err(msg!("err.tunnel.sshUserRequired", "SSH 用户名不能为空"));
        }
        if !valid_port(self.ssh_port) {
            return Err(msg!("err.tunnel.sshPortInvalid", "SSH 端口无效: {port}", port = self.ssh_port));
        }
        if self.auth_type == AuthType::Key && self.key_file.is_empty() {
            return Err(msg!("err.tunnel.keyFileRequired", "私钥认证需要选择私钥文件"));
        }
        // 端口 0 表示由系统分配，只允许本机监听时使用。
        if !(0..=65535).contains(&self.listen_port) || (self.kind == TunnelType::Remote && self.listen_port == 0) {
            return Err(msg!("err.tunnel.listenPortInvalid", "监听端口无效: {port}", port = self.listen_port));
        }
        if self.kind == TunnelType::Dynamic {
            if self.proxy_user.is_empty() != self.proxy_pass.is_empty() {
                return Err(msg!("err.tunnel.proxyAuthPair", "代理用户名和密码必须同时填写"));
            }
            if !is_loopback(&self.listen_host) && self.proxy_user.is_empty() {
                return Err(msg!(
                    "err.tunnel.proxyAuthRequired",
                    "代理监听在 {host}，不是本机地址，必须设置代理用户名和密码",
                    host = self.listen_host
                ));
            }
        } else {
            if self.target_host.is_empty() {
                return Err(msg!("err.tunnel.targetHostRequired", "目标地址不能为空"));
            }
            if !valid_port(self.target_port) {
                return Err(msg!("err.tunnel.targetPortInvalid", "目标端口无效: {port}", port = self.target_port));
            }
        }
        Ok(())
    }

    pub fn ssh_addr(&self) -> String {
        join_host_port(&self.ssh_host, self.ssh_port)
    }

    pub fn listen_addr(&self) -> String {
        join_host_port(&self.listen_host, self.listen_port)
    }

    pub fn target_addr(&self) -> String {
        join_host_port(&self.target_host, self.target_port)
    }

    pub fn clear_secrets(&mut self) {
        self.password.clear();
        self.key_passphrase.clear();
        self.proxy_pass.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelState {
    Stopped,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

impl TunnelState {
    pub fn label(self) -> &'static str {
        crate::i18n::pick(match self {
            TunnelState::Stopped => ["未启动", "未啟動", "Stopped"],
            TunnelState::Connecting => ["连接中", "連線中", "Connecting"],
            TunnelState::Connected => ["已连接", "已連線", "Connected"],
            TunnelState::Reconnecting => ["重连中", "重新連線中", "Reconnecting"],
            TunnelState::Failed => ["失败", "失敗", "Failed"],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub tunnel_id: String,
    pub state: TunnelState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Msg>,
    pub since: DateTime<Local>,
    pub attempt: u32,
    pub listen_addr: String,
}

impl TunnelStatus {
    pub fn stopped(t: &Tunnel) -> Self {
        TunnelStatus {
            tunnel_id: t.id.clone(),
            state: TunnelState::Stopped,
            error: None,
            since: Local::now(),
            attempt: 0,
            listen_addr: t.listen_addr(),
        }
    }
}

/// 返回给前端的隧道信息：不包含任何密码，只告诉前端是否已保存。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelView {
    #[serde(flatten)]
    pub tunnel: Tunnel,
    pub has_password: bool,
    pub has_key_passphrase: bool,
    pub has_proxy_pass: bool,
    pub status: TunnelStatus,
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub active_conns: usize,
}

/// 新建或修改隧道。编辑已有隧道时，密码类字段留空表示保持原值。
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SaveTunnelReq {
    #[serde(flatten)]
    pub tunnel: Tunnel,
    pub clear_password: bool,
    pub clear_key_passphrase: bool,
    pub clear_proxy_pass: bool,
}

/// 首次连接某台服务器时，请前端弹窗确认指纹。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyPrompt {
    pub request_id: String,
    pub tunnel_id: String,
    pub tunnel_name: String,
    pub host: String,
    pub key_type: String,
    pub fingerprint: String,
}

/// 每秒推送一次，速率单位为字节/秒。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Traffic {
    pub time: Option<DateTime<Local>>,
    pub up: u64,
    pub down: u64,
    /// 运行中隧道本次运行以来的上传、下载之和。
    pub total_up: u64,
    pub total_down: u64,
    pub active: usize,
    /// 后端进程占用的物理内存，不含界面 WebView 进程。
    pub memory: u64,
    pub tunnels: HashMap<String, TunnelTraffic>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TunnelTraffic {
    pub up: u64,
    pub down: u64,
    /// 本次运行以来的上传、下载。
    pub total_up: u64,
    pub total_down: u64,
    pub active: usize,
}

/// 连接页的一行。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionView {
    pub id: String,
    pub tunnel_id: String,
    pub tunnel_name: String,
    pub conn_id: u64,
    pub proto: String,
    pub client: String,
    pub target: String,
    pub start: DateTime<Local>,
    pub bytes_up: u64,
    pub bytes_down: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub seq: u64,
    pub time: DateTime<Local>,
    pub level: String,
    pub message: String,
    pub fields: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(kind: TunnelType) -> Tunnel {
        let mut t = Tunnel {
            kind,
            ssh_host: "example.com".into(),
            ssh_user: "root".into(),
            listen_port: 1080,
            target_host: "127.0.0.1".into(),
            target_port: 80,
            ..Default::default()
        };
        t.normalize();
        t
    }

    #[test]
    fn normalize_defaults() {
        let t = base(TunnelType::Local);
        assert_eq!(t.ssh_port, 22);
        assert_eq!(t.listen_host, "127.0.0.1");
        assert_eq!(t.keepalive_seconds, DEFAULT_KEEPALIVE);
        assert_eq!(t.name, "root@example.com:1080");
        assert!(t.validate().is_ok());
    }

    #[test]
    fn validate_rules() {
        let mut t = base(TunnelType::Remote);
        t.listen_port = 0;
        assert!(t.validate().is_err(), "远程转发不允许端口 0");

        let mut t = base(TunnelType::Dynamic);
        t.listen_host = "0.0.0.0".into();
        assert_eq!(t.validate().unwrap_err().i18n, "err.tunnel.proxyAuthRequired");
        t.proxy_user = "u".into();
        assert!(t.validate().is_err(), "用户名密码要同时填写");
        t.proxy_pass = "p".into();
        assert!(t.validate().is_ok());

        let mut t = base(TunnelType::Local);
        t.auth_type = AuthType::Key;
        assert!(t.validate().is_err(), "私钥认证必须选择私钥文件");
    }

    #[test]
    fn json_round_trip_without_secrets() {
        let mut t = base(TunnelType::Dynamic);
        t.password = "secret".into();
        t.clear_secrets();
        let s = serde_json::to_string(&t).unwrap();
        assert!(!s.contains("\"password\":") && !s.contains("secret"));
        assert!(s.contains("\"type\":\"dynamic\""));
        let back: Tunnel = serde_json::from_str(&s).unwrap();
        assert_eq!(back, t);
    }
}
