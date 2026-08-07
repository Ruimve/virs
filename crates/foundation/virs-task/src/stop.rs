use std::future::Future;

use tokio_util::sync::CancellationToken;

/* 任务取消令牌封装：包装 tokio_util 的 CancellationToken，为任务提供统一的取消信号机制 */
pub struct Stop(CancellationToken);

impl Stop {
    pub(crate) fn new() -> Self {
        Stop(CancellationToken::new())
    }

    /* 克隆取消令牌，子任务持有克隆后可监听取消信号 */
    pub(crate) fn clone(&self) -> Self {
        Stop(self.0.clone())
    }

    /* 触发取消信号，所有持有该令牌克隆的任务将收到取消通知 */
    pub(crate) fn cancel(&self) {
        self.0.cancel();
    }

    /* 返回一个在取消信号触发时完成的 Future，用于 select! 中监听取消 */
    pub fn cancelled(&self) -> impl Future<Output = ()> + '_ {
        self.0.cancelled()
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}
