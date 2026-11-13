//! 多语言支持。
//!
//! 返回给前端的内容只包含数据和国际化 key（[`Msg`]），文案由前端按当前语言翻译，见 src/locales。
//! [`Msg`] 里的中文只用于日志，不会序列化给前端。
//! 托盘菜单、文件对话框这类由后端直接绘制的原生界面用 [`pick`] / [`tr!`] 按设置的语言显示。

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::Serialize;

/// 国际化消息：前端用 i18n 查文案，用 params 替换文案中的 {name} 占位符。
/// 参数名为 cause 时表示底层错误原文，文案中没有 {cause} 占位符时由前端追加在末尾。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Msg {
    pub i18n: &'static str,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<&'static str, Param>,
    #[serde(skip)]
    text: String,
}

/// 消息参数：普通文本，或需要前端再翻译一次的嵌套消息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Param {
    Text(String),
    Msg(Box<Msg>),
}

impl Msg {
    pub fn new(i18n: &'static str, text: String, params: Vec<(&'static str, Param)>) -> Self {
        Msg { i18n, params: params.into_iter().collect(), text }
    }

    /// 中文文案，用于日志和测试断言。
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 追加底层错误原文。
    pub fn with_cause(mut self, cause: impl fmt::Display) -> Self {
        let cause = cause.to_string();
        self.text = format!("{}: {cause}", self.text);
        self.params.insert("cause", Param::Text(cause));
        self
    }
}

impl fmt::Display for Param {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Param::Text(s) => f.write_str(s),
            Param::Msg(m) => f.write_str(&m.text),
        }
    }
}

/// [`msg!`] 参数的转换：实现 Display 的值转为文本，[`Msg`] 作为嵌套消息。
pub trait ToParam {
    fn to_param(&self) -> Param;
}

impl<T: fmt::Display + ?Sized> ToParam for T {
    fn to_param(&self) -> Param {
        Param::Text(self.to_string())
    }
}

impl ToParam for Msg {
    fn to_param(&self) -> Param {
        Param::Msg(Box::new(self.clone()))
    }
}

/// 构造 [`Msg`]：`msg!("err.ssh.connectFailed", "连接 {addr} 失败: {cause}", addr, cause = e)`。
/// 中文模板中的占位符必须都作为参数传入，参数名即前端文案中的占位符名。
#[macro_export]
macro_rules! msg {
    ($key:literal, $zh:literal $(, $name:ident $(= $val:expr)?)* $(,)?) => {{
        $( let $name = $crate::i18n::ToParam::to_param(&$crate::__msg_val!($name $(= $val)?)); )*
        $crate::i18n::Msg::new($key, format!($zh $(, $name = $name)*), vec![$((stringify!($name), $name)),*])
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __msg_val {
    ($name:ident) => {
        $name
    };
    ($name:ident = $val:expr) => {
        $val
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Lang {
    ZhCn = 0,
    ZhTw = 1,
    En = 2,
}

impl Lang {
    pub fn tag(self) -> &'static str {
        match self {
            Lang::ZhCn => "zh-CN",
            Lang::ZhTw => "zh-TW",
            Lang::En => "en",
        }
    }
}

pub const LANGUAGES: [&str; 4] = ["system", "zh-CN", "zh-TW", "en"];

static CURRENT: AtomicU8 = AtomicU8::new(Lang::ZhCn as u8);

pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::ZhTw,
        2 => Lang::En,
        _ => Lang::ZhCn,
    }
}

/// 系统首选语言对应的支持语言，不支持的系统语言一律用英文。
/// 前端“跟随系统”也用这个结果，保证界面与托盘菜单语言一致。
pub fn system() -> Lang {
    sys_locale::get_locale().map(|l| from_tag(&l)).unwrap_or(Lang::En)
}

/// 按配置值（system/zh-CN/zh-TW/en）切换原生界面的语言。
pub fn set(language: &str) {
    let lang = match language {
        "system" => system(),
        tag => from_tag(tag),
    };
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

/// 把 BCP 47 语言标签映射到支持的语言：繁体地区和 Hant 用繁体中文，其余中文用简体，其他语言用英文。
fn from_tag(tag: &str) -> Lang {
    let t = tag.replace('_', "-").to_ascii_lowercase();
    if !(t == "zh" || t.starts_with("zh-")) {
        return Lang::En;
    }
    if t.contains("hant") || ["-tw", "-hk", "-mo"].iter().any(|r| t.ends_with(r) || t.contains(&format!("{r}-"))) {
        Lang::ZhTw
    } else {
        Lang::ZhCn
    }
}

/// 原生界面文案：按当前语言从 [简体, 繁體, English] 中选一个。
pub fn pick(texts: [&'static str; 3]) -> &'static str {
    texts[current() as usize]
}

/// 原生界面文案的格式化版本：`tr!("简体 {x}", "繁體 {x}", "English {x}")`。
#[macro_export]
macro_rules! tr {
    ($zh:literal, $tw:literal, $en:literal $(; $($arg:tt)*)?) => {
        match $crate::i18n::current() {
            $crate::i18n::Lang::ZhCn => format!($zh $(, $($arg)*)?),
            $crate::i18n::Lang::ZhTw => format!($tw $(, $($arg)*)?),
            $crate::i18n::Lang::En => format!($en $(, $($arg)*)?),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags() {
        assert_eq!(from_tag("zh-CN"), Lang::ZhCn);
        assert_eq!(from_tag("zh"), Lang::ZhCn);
        assert_eq!(from_tag("zh-Hans-SG"), Lang::ZhCn);
        assert_eq!(from_tag("zh-TW"), Lang::ZhTw);
        assert_eq!(from_tag("zh_HK"), Lang::ZhTw);
        assert_eq!(from_tag("zh-Hant"), Lang::ZhTw);
        assert_eq!(from_tag("zh-MO-x"), Lang::ZhTw);
        assert_eq!(from_tag("en-US"), Lang::En);
        assert_eq!(from_tag("ja-JP"), Lang::En);
    }

    #[test]
    fn msg_serializes_key_and_params_only() {
        let what = msg!("err.ssh.denied.password", "密码");
        let port = 22;
        let m = msg!("err.test", "端口 {port} 被{what}拒绝", port, what).with_cause("boom");
        assert_eq!(m.text(), "端口 22 被密码拒绝: boom");
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "i18n": "err.test",
                "params": { "port": "22", "what": { "i18n": "err.ssh.denied.password" }, "cause": "boom" }
            })
        );
    }
}
