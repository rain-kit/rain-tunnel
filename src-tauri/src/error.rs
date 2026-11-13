use std::fmt;

use serde::Serialize;

use crate::i18n::Msg;

/// 返回给前端的错误，序列化为 `{ code, i18n, params }`：code 区分错误类型，i18n 和 params 由前端翻译成文案。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ApiError {
    pub code: u32,
    #[serde(flatten)]
    pub msg: Msg,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "错误码:{},消息:{},i18n:{}", self.code, self.msg.text(), self.msg.i18n)
    }
}

impl std::error::Error for ApiError {}

pub type ApiResult<T> = Result<T, ApiError>;

/// 错误码定义：码值、国际化 key、中文说明（只用于日志）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Code(pub u32, pub &'static str, pub &'static str);

impl Code {
    pub fn err(self) -> ApiError {
        ApiError { code: self.0, msg: Msg::new(self.1, self.2.to_string(), Vec::new()) }
    }

    /// 附带底层错误原文，前端显示为“错误码文案: 原文”。
    pub fn cause(self, cause: impl fmt::Display) -> ApiError {
        let mut e = self.err();
        e.msg = e.msg.with_cause(cause);
        e
    }

    /// 用更具体的消息代替错误码本身的文案。
    pub fn msg(self, msg: Msg) -> ApiError {
        ApiError { code: self.0, msg }
    }
}

impl From<Code> for ApiError {
    fn from(c: Code) -> Self {
        c.err()
    }
}

// -------------------- 系统 -------------------- //
pub const SERVICE_ERROR: Code = Code(1, "err.service", "系统异常");
pub const PARAM_INVALID: Code = Code(2, "err.paramInvalid", "请求参数错误");
pub const OPEN_DIR_FAILED: Code = Code(3, "err.openDirFailed", "打开目录失败");
pub const OPEN_URL_FAILED: Code = Code(4, "err.openUrlFailed", "打开链接失败");

// -------------------- 隧道 100000 -------------------- //
pub const TUNNEL_NOT_FOUND: Code = Code(100001, "err.tunnelNotFound", "隧道不存在");
pub const TUNNEL_INVALID: Code = Code(100002, "err.tunnelInvalid", "隧道配置无效");
pub const TUNNEL_START_FAILED: Code = Code(100003, "err.tunnelStartFailed", "隧道启动失败");
pub const TUNNEL_FILE_INVALID: Code = Code(100004, "err.tunnelFileInvalid", "tunnels.json 格式错误");
pub const SECRET_STORE_FAILED: Code = Code(100005, "err.secretStoreFailed", "保存密码到系统凭据库失败");
pub const HOST_KEY_EXPIRED: Code = Code(100006, "err.hostKeyExpired", "指纹确认请求已过期");
pub const CONN_NOT_FOUND: Code = Code(100007, "err.connNotFound", "连接不存在或已关闭");
pub const TUNNEL_LIST_FAILED: Code = Code(100008, "err.tunnelListFailed", "读取隧道列表失败");
pub const TUNNEL_SAVE_FAILED: Code = Code(100009, "err.tunnelSaveFailed", "保存隧道失败");
pub const SECRET_READ_FAILED: Code = Code(100010, "err.secretReadFailed", "读取系统凭据库失败");

// -------------------- 设置 200000 -------------------- //
pub const SETTING_SAVE_FAILED: Code = Code(200001, "err.settingSaveFailed", "保存设置失败");
pub const AUTOSTART_FAILED: Code = Code(200002, "err.autostartFailed", "设置开机自启失败");
pub const UPDATE_CHECK_FAILED: Code = Code(200003, "err.updateCheckFailed", "检查更新失败");
pub const UPDATE_INSTALL_FAILED: Code = Code(200004, "err.updateInstallFailed", "更新失败");
pub const UPDATE_NOT_AVAILABLE: Code = Code(200005, "err.updateNotAvailable", "没有可直接安装的新版本，请前往下载页更新");
pub const UPDATE_TUNNEL_UNAVAILABLE: Code = Code(200006, "err.updateTunnelUnavailable", "所选隧道未连接，请换一种方式重试");
