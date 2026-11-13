use std::sync::Arc;
use std::time::Duration;

use russh::MethodKind;
use russh::client::{AuthResult, Handle, KeyboardInteractiveAuthResponse};
use russh::keys::agent::AgentIdentity;
use russh::keys::agent::client::{AgentClient, AgentStream};
use russh::keys::{PrivateKey, PrivateKeyWithHashAlg};

use super::{ClientHandler, SshError};
use crate::i18n::Msg;
use crate::model::{AuthType, Tunnel};
use crate::util::expand_home;

type Agent = AgentClient<Box<dyn AgentStream + Send + Unpin + 'static>>;

/// 连接前准备好的认证材料。私钥在连接前加载，读不了就直接失败，不用白连一次服务器。
pub enum AuthMethod {
    Password(String),
    Key(Arc<PrivateKey>),
    Agent(Agent),
}

pub async fn prepare(t: &Tunnel) -> Result<AuthMethod, SshError> {
    match t.auth_type {
        AuthType::Password => Ok(AuthMethod::Password(t.password.clone())),
        AuthType::Key => load_key(&t.key_file, &t.key_passphrase).map(|k| AuthMethod::Key(Arc::new(k))),
        AuthType::Agent => connect_agent().await.map(AuthMethod::Agent),
    }
}

fn load_key(path: &str, passphrase: &str) -> Result<PrivateKey, SshError> {
    let full = expand_home(path);
    let path = full.display();
    let text = std::fs::read_to_string(&full)
        .map_err(|e| SshError::Permanent(msg!("err.ssh.keyReadFailed", "读取私钥 {path} 失败", path).with_cause(e)))?;
    match russh::keys::decode_secret_key(&text, None) {
        Ok(k) => Ok(k),
        Err(russh::keys::Error::KeyIsEncrypted) if passphrase.is_empty() => {
            Err(SshError::Permanent(msg!("err.ssh.keyEncrypted", "私钥已加密，需要填写私钥口令")))
        }
        Err(russh::keys::Error::KeyIsEncrypted) => russh::keys::decode_secret_key(&text, Some(passphrase))
            .map_err(|e| SshError::Permanent(msg!("err.ssh.keyDecryptFailed", "解密私钥失败（口令错误？）").with_cause(e))),
        Err(e) => Err(SshError::Permanent(msg!("err.ssh.keyParseFailed", "解析私钥 {path} 失败", path).with_cause(e))),
    }
}

/// 判断私钥文件是否加密；读取或解析失败时返回 false，由连接时报告具体错误。
pub fn key_needs_passphrase(path: &str) -> bool {
    std::fs::read_to_string(expand_home(path))
        .is_ok_and(|text| matches!(russh::keys::decode_secret_key(&text, None), Err(russh::keys::Error::KeyIsEncrypted)))
}

#[cfg(windows)]
async fn connect_agent() -> Result<Agent, SshError> {
    // Windows 自带的 OpenSSH agent 走命名管道；若 SSH_AUTH_SOCK 指向其他管道（如 Pageant 的 OpenSSH 兼容管道）则优先使用。
    let pipe = std::env::var("SSH_AUTH_SOCK")
        .ok()
        .filter(|s| s.starts_with(r"\\.\pipe\"))
        .unwrap_or_else(|| r"\\.\pipe\openssh-ssh-agent".to_string());
    match tokio::time::timeout(Duration::from_secs(5), AgentClient::connect_named_pipe(&pipe)).await {
        Ok(Ok(c)) => Ok(c.dynamic()),
        Ok(Err(e)) => Err(SshError::Other(msg!(
            "err.ssh.agentPipeFailed",
            "连接 ssh-agent 失败（{pipe}）: {cause}。请确认已启动 OpenSSH Authentication Agent 服务并用 ssh-add 加载了密钥",
            pipe,
            cause = e
        ))),
        Err(_) => Err(agent_timeout()),
    }
}

fn agent_timeout() -> SshError {
    SshError::Other(msg!("err.ssh.agentTimeout", "连接 ssh-agent 超时"))
}

#[cfg(not(windows))]
async fn connect_agent() -> Result<Agent, SshError> {
    let sock = std::env::var("SSH_AUTH_SOCK")
        .map_err(|_| SshError::Other(msg!("err.ssh.agentNoSock", "连接 ssh-agent 失败: 环境变量 SSH_AUTH_SOCK 未设置")))?;
    match tokio::time::timeout(Duration::from_secs(5), AgentClient::connect_uds(&sock)).await {
        Ok(Ok(c)) => Ok(c.dynamic()),
        Ok(Err(e)) => Err(SshError::Other(msg!("err.ssh.agentConnectFailed", "连接 ssh-agent 失败").with_cause(e))),
        Err(_) => Err(agent_timeout()),
    }
}

fn denied(what: Msg) -> SshError {
    SshError::Permanent(msg!("err.ssh.authDenied", "SSH 认证失败：服务器拒绝了{what}", what))
}

fn denied_password() -> SshError {
    denied(msg!("err.ssh.denied.password", "密码（用户名或密码错误）"))
}

pub async fn authenticate(handle: &mut Handle<ClientHandler>, user: &str, method: AuthMethod) -> Result<(), SshError> {
    match method {
        AuthMethod::Password(pw) => password(handle, user, pw).await,
        AuthMethod::Key(key) => {
            let hash = rsa_hash(handle, &key).await?;
            match handle.authenticate_publickey(user, PrivateKeyWithHashAlg::new(key, hash)).await? {
                AuthResult::Success => Ok(()),
                AuthResult::Failure { .. } => Err(denied(msg!(
                    "err.ssh.denied.key",
                    "该私钥（用户名错误，或公钥未加入服务器的 authorized_keys）"
                ))),
            }
        }
        AuthMethod::Agent(mut agent) => {
            let identities = agent
                .request_identities()
                .await
                .map_err(|e| SshError::Other(msg!("err.ssh.agentListFailed", "读取 ssh-agent 中的密钥失败").with_cause(e)))?;
            let keys: Vec<_> = identities
                .into_iter()
                .filter_map(|id| match id {
                    AgentIdentity::PublicKey { key, .. } => Some(key),
                    _ => None,
                })
                .collect();
            if keys.is_empty() {
                return Err(SshError::Permanent(msg!("err.ssh.agentNoKeys", "ssh-agent 中没有可用的密钥，请先用 ssh-add 加载")));
            }
            for key in keys {
                let hash = if key.algorithm().is_rsa() {
                    handle.best_supported_rsa_hash().await?.flatten()
                } else {
                    None
                };
                match handle.authenticate_publickey_with(user, key, hash, &mut agent).await {
                    Ok(AuthResult::Success) => return Ok(()),
                    Ok(AuthResult::Failure { .. }) => continue,
                    Err(e) => return Err(SshError::Other(msg!("err.ssh.agentSignFailed", "ssh-agent 签名失败").with_cause(e))),
                }
            }
            Err(denied(msg!("err.ssh.denied.agent", "ssh-agent 中的全部密钥")))
        }
    }
}

async fn rsa_hash(handle: &Handle<ClientHandler>, key: &PrivateKey) -> Result<Option<russh::keys::HashAlg>, SshError> {
    if key.algorithm().is_rsa() {
        Ok(handle.best_supported_rsa_hash().await?.flatten())
    } else {
        Ok(None)
    }
}

/// 先试 password；服务器只开了 keyboard-interactive 时，用同一个密码回答所有问题。
async fn password(handle: &mut Handle<ClientHandler>, user: &str, pw: String) -> Result<(), SshError> {
    let remaining = match handle.authenticate_password(user, pw.clone()).await? {
        AuthResult::Success => return Ok(()),
        AuthResult::Failure { remaining_methods, .. } => remaining_methods,
    };
    if !remaining.contains(&MethodKind::KeyboardInteractive) {
        return Err(denied_password());
    }
    let mut resp = handle.authenticate_keyboard_interactive_start(user, None).await?;
    for _ in 0..8 {
        match resp {
            KeyboardInteractiveAuthResponse::Success => return Ok(()),
            KeyboardInteractiveAuthResponse::Failure { .. } => break,
            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                let answers = vec![pw.clone(); prompts.len()];
                resp = handle.authenticate_keyboard_interactive_respond(answers).await?;
            }
        }
    }
    Err(denied_password())
}
