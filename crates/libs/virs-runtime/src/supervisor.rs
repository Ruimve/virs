use std::future::Future;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::error;

use crate::CancellationToken;

/// 受监督的任务句柄 — 包装 JoinHandle 并注册到 TaskSupervisor。
///
/// 当 TaskSupervisor::shutdown 被调用时，所有已注册的任务会收到取消信号，
/// 随后在超时窗口内等待退出；超时则 abort。
pub struct SupervisedHandle {
    name: String,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl SupervisedHandle {
    /// 等待任务自然结束（不发送取消信号）。
    pub async fn join(&self) {
        if let Some(h) = self.handle.lock().await.take() {
            let _ = h.await;
        }
    }

    /// 等待任务退出，超时则 abort。消费 self。
    pub async fn abort_with_timeout(self, timeout: Duration) {
        let handle = self.handle.lock().await.take();
        if let Some(h) = handle {
            let abort_handle = h.abort_handle();
            tokio::select! {
                result = h => {
                    match result {
                        Ok(()) => {}
                        Err(e) if e.is_panic() => {
                            error!(task = %self.name, "supervised task panicked");
                        }
                        Err(_) => {
                            error!(task = %self.name, "supervised task was cancelled");
                        }
                    }
                }
                _ = tokio::time::sleep(timeout) => {
                    error!(task = %self.name, "supervised task timed out, aborting");
                    abort_handle.abort();
                }
            }
        }
    }
}

/// 任务监督器 — 统一管理多个后台任务的 JoinHandle 和取消信号。
///
/// 每个组件（如 KlineEngine、PositionEngine、AutoWorker）持有一个 TaskSupervisor 实例，
/// 所有 `tokio::spawn` 通过 `spawn()` 方法注册，确保：
/// 1. 每个任务都有 JoinHandle（解决 fire-and-forget 问题）
/// 2. 每个任务都监听取消信号（解决 AtomicBool 不可中断 sleep 问题）
/// 3. `shutdown()` 统一发送取消信号 + 等待全部退出 + 超时 abort
pub struct TaskSupervisor {
    cancel: CancellationToken,
    tasks: Mutex<Vec<SupervisedHandle>>,
    shutdown_timeout: Duration,
}

impl TaskSupervisor {
    /// 创建一个监督器，使用给定的取消令牌和默认 5 秒关闭超时。
    #[must_use]
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
            tasks: Mutex::new(Vec::new()),
            shutdown_timeout: Duration::from_secs(5),
        }
    }

    /// 获取此监督器的取消令牌。spawn 的任务应监听此令牌的 `cancelled()`。
    pub fn cancel(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// 获取此监督器的取消令牌引用（用于 child_token 等）。
    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.cancel
    }

    /// 注册并 spawn 一个受监督的后台任务。
    ///
    /// 任务 Future 自动与 `cancel.cancelled()` 竞争：取消信号到达时立即退出。
    /// 适用于不需要自行管理取消逻辑的简单任务。
    pub async fn spawn<F, Fut>(&self, name: &str, f: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let cancel = self.cancel.clone();
        let task_name = name.to_string();
        let log_name = task_name.clone();
        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::debug!(task = %log_name, "task cancelled");
                }
                _ = f() => {
                    tracing::debug!(task = %log_name, "task completed");
                }
            }
        });

        self.register(task_name, handle).await;
    }

    /// 注册并 spawn 一个不受取消信号自动包裹的任务。
    ///
    /// 适用于任务内部已自行管理取消逻辑的场景（如 WsManager 内部的复杂状态机）。
    /// 仍然保存 JoinHandle 以支持 shutdown 等待。
    pub async fn spawn_raw<F, Fut>(&self, name: &str, f: F)
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let cancel = self.cancel.clone();
        let task_name = name.to_string();
        let handle = tokio::spawn(f(cancel));

        self.register(task_name, handle).await;
    }

    async fn register(&self, name: String, handle: JoinHandle<()>) {
        let supervised = SupervisedHandle {
            name,
            handle: Mutex::new(Some(handle)),
        };
        self.tasks.lock().await.push(supervised);
    }

    /// 优雅关闭：发送取消信号 → 并发等待全部任务退出 → 超时 abort。
    pub async fn shutdown(&self) {
        self.cancel.cancel();
        let tasks: Vec<SupervisedHandle> = self.tasks.lock().await.drain(..).collect();
        let timeout = self.shutdown_timeout;
        // 并发等待所有任务，总超时 = shutdown_timeout 而非 N × shutdown_timeout
        let mut join_set = tokio::task::JoinSet::new();
        for task in tasks {
            join_set.spawn(task.abort_with_timeout(timeout));
        }
        while join_set.join_next().await.is_some() {}
    }

    /// 使用指定超时时间进行优雅关闭。
    pub async fn shutdown_with_timeout(&self, timeout: Duration) {
        self.cancel.cancel();
        let tasks: Vec<SupervisedHandle> = self.tasks.lock().await.drain(..).collect();
        let mut join_set = tokio::task::JoinSet::new();
        for task in tasks {
            join_set.spawn(task.abort_with_timeout(timeout));
        }
        while join_set.join_next().await.is_some() {}
    }

    /// 当前已注册的任务数量。
    pub async fn task_count(&self) -> usize {
        self.tasks.lock().await.len()
    }
}

/// 构建器，用于自定义 TaskSupervisor 的关闭超时等参数。
pub struct TaskSupervisorBuilder {
    cancel: CancellationToken,
    shutdown_timeout: Duration,
}

impl TaskSupervisorBuilder {
    #[must_use]
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
            shutdown_timeout: Duration::from_secs(5),
        }
    }

    pub fn shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }

    #[must_use]
    pub fn build(self) -> TaskSupervisor {
        TaskSupervisor {
            cancel: self.cancel,
            tasks: Mutex::new(Vec::new()),
            shutdown_timeout: self.shutdown_timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_spawn_and_shutdown() {
        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel);

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        supervisor
            .spawn("test_task", || async move {
                tokio::time::sleep(Duration::from_secs(60)).await;
                flag_clone.store(true, Ordering::Relaxed);
            })
            .await;

        assert_eq!(supervisor.task_count().await, 1);
        supervisor.shutdown().await;
        assert_eq!(supervisor.task_count().await, 0);
        // Task should have been cancelled before completing
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_spawn_raw_manual_cancel() {
        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel.clone());

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        supervisor
            .spawn_raw("manual_cancel_task", |cancel| async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        _ = interval.tick() => {
                            flag_clone.store(true, Ordering::Relaxed);
                        }
                    }
                }
            })
            .await;

        supervisor.shutdown().await;
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_task_completes_naturally() {
        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel);

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        supervisor
            .spawn("completing_task", || async move {
                flag_clone.store(true, Ordering::Relaxed);
            })
            .await;

        // Wait for task to complete
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(flag.load(Ordering::Relaxed));
        supervisor.shutdown().await;
    }
}
