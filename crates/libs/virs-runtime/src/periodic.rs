use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tracing::error;

use crate::{CancellationToken, TaskSupervisor};

/// 周期任务原语 — 封装 `tokio::time::interval` + 取消信号 + panic 恢复。
///
/// # 特性
///
/// - 首次触发可控：`first_tick_immediate(true)` 在启动时立即触发一次，否则延迟一个间隔
/// - 可中断：`interval.tick()` 被 `cancel.cancelled()` 包裹，关闭时立即退出
/// - panic 隔离：单次执行 panic 不会终止整个任务，记录 error 后继续下一周期
/// - 受监督：JoinHandle 注册到 TaskSupervisor，shutdown 时统一管理
///
/// # 示例
///
/// ```ignore
/// use virs_runtime::{CancellationToken, TaskSupervisor, PeriodicTask};
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let cancel = CancellationToken::root();
/// let supervisor = TaskSupervisor::new(cancel);
///
/// PeriodicTask::builder("gap_detector")
///     .interval(Duration::from_secs(60))
///     .first_tick_immediate(false)
///     .spawn(&supervisor, || async {
///         println!("checking gaps...");
///     })
///     .await;
/// # });
/// ```
pub struct PeriodicTask {
    name: String,
    interval: Duration,
    first_tick_immediate: bool,
}

impl PeriodicTask {
    /// 创建构建器。
    #[must_use]
    pub fn builder(name: &str) -> PeriodicTaskBuilder {
        PeriodicTaskBuilder {
            name: name.to_string(),
            interval: Duration::from_secs(60),
            first_tick_immediate: false,
        }
    }

    /// Spawn 周期任务。闭包返回的 Future 在每次 tick 时执行。
    ///
    /// 闭包是 `Fn`（不可变借用），因此可在多个 tick 间共享捕获的环境。
    /// 如需可变状态，请在闭包内使用 `Arc<Mutex<T>>`。
    pub async fn spawn<F, Fut>(&self, supervisor: &TaskSupervisor, f: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let name = self.name.clone();
        let interval_dur = self.interval;
        let first_immediate = self.first_tick_immediate;
        let f = Arc::new(f);

        let spawn_name = name.clone();
        supervisor
            .spawn_raw(&spawn_name, move |cancel: CancellationToken| {
                let f = Arc::clone(&f);
                let log_name = name;
                async move {
                    let mut tick = if first_immediate {
                        tokio::time::interval(interval_dur)
                    } else {
                        let start = tokio::time::Instant::now() + interval_dur;
                        tokio::time::interval_at(start, interval_dur)
                    };

                    loop {
                        tokio::select! {
                            _ = cancel.cancelled() => break,
                            _ = tick.tick() => {
                                // panic 恢复：通过 tokio::spawn 隔离单次执行，
                                // panic 不会终止整个周期任务
                                let f = Arc::clone(&f);
                                let handle = tokio::spawn(f());
                                if let Err(join_err) = handle.await {
                                    if join_err.is_panic() {
                                        error!(
                                            task = %log_name,
                                            "periodic task panic recovered, continuing"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .await;
    }
}

/// 周期任务构建器。
pub struct PeriodicTaskBuilder {
    name: String,
    interval: Duration,
    first_tick_immediate: bool,
}

impl PeriodicTaskBuilder {
    /// 设置周期间隔。默认 60 秒。
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// 是否在启动时立即触发第一次执行。
    /// - `true`：spawn 后立即执行一次，然后按 interval 周期执行
    /// - `false`（默认）：延迟一个完整 interval 后首次执行
    pub fn first_tick_immediate(mut self, immediate: bool) -> Self {
        self.first_tick_immediate = immediate;
        self
    }

    /// 构建并 spawn 周期任务。
    pub async fn spawn<F, Fut>(self, supervisor: &TaskSupervisor, f: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let task = PeriodicTask {
            name: self.name,
            interval: self.interval,
            first_tick_immediate: self.first_tick_immediate,
        };
        task.spawn(supervisor, f).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Instant;

    #[tokio::test]
    async fn test_periodic_first_tick_immediate() {
        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel);

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        PeriodicTask::builder("test_immediate")
            .interval(Duration::from_millis(50))
            .first_tick_immediate(true)
            .spawn(&supervisor, move || {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                }
            })
            .await;

        // Wait for at least 2 ticks
        tokio::time::sleep(Duration::from_millis(120)).await;
        supervisor.shutdown().await;

        let count = counter.load(Ordering::Relaxed);
        assert!(count >= 2, "expected at least 2 ticks, got {count}");
    }

    #[tokio::test]
    async fn test_periodic_first_tick_delayed() {
        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel);

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        PeriodicTask::builder("test_delayed")
            .interval(Duration::from_millis(100))
            .first_tick_immediate(false)
            .spawn(&supervisor, move || {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                }
            })
            .await;

        // After 50ms, no tick should have occurred (delayed start)
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::Relaxed), 0);

        // After 150ms, at least 1 tick should have occurred
        tokio::time::sleep(Duration::from_millis(120)).await;
        supervisor.shutdown().await;

        let count = counter.load(Ordering::Relaxed);
        assert!(count >= 1, "expected at least 1 tick, got {count}");
    }

    #[tokio::test]
    async fn test_periodic_cancel_interrupts_tick() {
        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel.clone());

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        PeriodicTask::builder("test_cancel")
            .interval(Duration::from_secs(60))
            .first_tick_immediate(true)
            .spawn(&supervisor, move || {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                }
            })
            .await;

        // First tick fires immediately
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Cancel should interrupt the 60s wait for next tick
        let start = Instant::now();
        supervisor.shutdown().await;
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(2),
            "shutdown should be fast, took {elapsed:?}"
        );
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }
}
