use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use crate::core::stats::Conn;

const BUF_SIZE: usize = 32 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("空闲超时")]
    Idle,
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// 记录最近一次读写时间，用于空闲超时。
pub struct Activity {
    start: Instant,
    last_ms: AtomicU64,
}

impl Activity {
    pub fn new() -> Self {
        Activity { start: Instant::now(), last_ms: AtomicU64::new(0) }
    }
}

impl Default for Activity {
    fn default() -> Self {
        Self::new()
    }
}

impl Activity {
    pub fn touch(&self) {
        self.last_ms.store(self.start.elapsed().as_millis() as u64, Ordering::Relaxed);
    }

    /// 超过 idle 时长没有活动时返回。
    pub async fn idle(&self, idle: Duration) {
        loop {
            let last = Duration::from_millis(self.last_ms.load(Ordering::Relaxed));
            let since = self.start.elapsed().saturating_sub(last);
            if since >= idle {
                return;
            }
            tokio::time::sleep(idle - since).await;
        }
    }

    /// idle 为 None 时永远不返回。
    pub async fn idle_opt(&self, idle: Option<Duration>) {
        match idle {
            Some(d) => self.idle(d).await,
            None => std::future::pending().await,
        }
    }
}

/// 单向拷贝并计数；读到 EOF 时关闭写方向（半关闭），让另一方向继续传完。
pub async fn pipe<R, W>(r: &mut R, w: &mut W, count: impl Fn(u64), act: &Activity) -> io::Result<()>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    let mut buf = vec![0u8; BUF_SIZE];
    loop {
        let n = r.read(&mut buf).await?;
        if n == 0 {
            let _ = w.shutdown().await;
            return Ok(());
        }
        act.touch();
        w.write_all(&buf[..n]).await?;
        count(n as u64);
    }
}

/// 双向转发 client 与 target 之间的数据，直到两个方向都结束、任一方向出错或空闲超时。
pub async fn relay<A, B>(client: A, target: B, conn: &Conn, idle: Option<Duration>) -> Result<(), RelayError>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let (mut cr, mut cw) = tokio::io::split(client);
    let (mut tr, mut tw) = tokio::io::split(target);
    let act = Activity::new();
    let up = pipe(&mut cr, &mut tw, |n| conn.add_up(n), &act);
    let down = pipe(&mut tr, &mut cw, |n| conn.add_down(n), &act);
    let result = tokio::select! {
        r = async { tokio::try_join!(up, down) } => r.map(|_| ()).map_err(RelayError::from),
        _ = act.idle_opt(idle) => Err(RelayError::Idle),
    };
    match result {
        Err(RelayError::Io(e)) if is_closed_err(&e) => Ok(()),
        r => r,
    }
}

/// 连接被对端或本端关闭导致的错误，不需要报告。
pub fn is_closed_err(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::UnexpectedEof
            | io::ErrorKind::NotConnected
    )
}
