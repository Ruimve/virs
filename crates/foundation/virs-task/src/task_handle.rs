use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::error;

use crate::Stop;

/* 任务优雅关闭的默认超时时间：超过此时间仍未结束则强制 abort */
const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/* 异步任务句柄：封装 JoinHandle 和取消令牌，提供取消、等待和超时终止能力 */
pub struct TaskHandle {
    stop: Stop,
    handle: Option<JoinHandle<()>>,
}

impl TaskHandle {
    pub(crate) fn new(stop: Stop, handle: JoinHandle<()>) -> Self {
        TaskHandle {
            stop,
            handle: Some(handle),
        }
    }

    pub fn cancel(&self) {
        self.stop.cancel();
    }

    pub async fn join(self) {
        self.join_with_timeout(DEFAULT_SHUTDOWN_TIMEOUT).await;
    }

    /* 等待任务结束，超时后强制中止：先尝试优雅等待，超时则调用 abort 终止 */
    pub async fn join_with_timeout(mut self, timeout: Duration) {
        if let Some(h) = self.handle.take() {
            let abort_handle = h.abort_handle();
            match tokio::time::timeout(timeout, h).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    if e.is_panic() {
                        error!("task panicked");
                    }
                }
                Err(_) => {
                    /* 超时仍未退出，强制终止任务 */
                    abort_handle.abort();
                    error!("task timed out, aborted");
                }
            }
        }
    }
}

/* TaskHandle 被 drop 时自动触发取消信号，确保任务不会泄漏 */
impl Drop for TaskHandle {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
