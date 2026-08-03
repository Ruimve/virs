use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tracing::error;

use crate::{CancellationToken, TaskSupervisor};

/// 退避策略。
#[derive(Debug, Clone, Copy)]
pub enum BackoffStrategy {
    /// 指数退避：`delay = initial * multiplier^attempt`，封顶于 `max_delay`
    Exponential { multiplier: f64 },
    /// 线性退避：`delay = initial * (attempt + 1)`，封顶于 `max_delay`
    Linear,
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        Self::Exponential { multiplier: 2.0 }
    }
}

/// 退避重试任务原语 — 封装指数/线性退避 + jitter + 熔断 + 取消信号。
///
/// # 特性
///
/// - 可中断退避：`sleep` 被 `cancel.cancelled()` 包裹，关闭时立即退出
/// - jitter：每次退避叠加 0~20% 随机抖动，防止雪崩
/// - 熔断：超过 `max_retries` 后停止并记录 error
/// - panic 隔离：单次执行 panic 不终止重试循环
/// - 受监督：JoinHandle 注册到 TaskSupervisor
///
/// # 示例
///
/// ```ignore
/// use virs_runtime::{CancellationToken, TaskSupervisor, RetryTask, BackoffStrategy};
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let cancel = CancellationToken::root();
/// let supervisor = TaskSupervisor::new(cancel);
///
/// RetryTask::builder("ws_reconnect")
///     .initial_delay(Duration::from_secs(1))
///     .max_delay(Duration::from_secs(60))
///     .max_retries(100)
///     .backoff(BackoffStrategy::Exponential { multiplier: 2.0 })
///     .spawn(&supervisor, |_attempt| async move {
///         // 尝试重连...
///         // 返回 Ok(()) 表示成功，Err(()) 表示需要重试
///         Result::<(), ()>::Ok(())
///     })
///     .await;
/// # });
/// ```
pub struct RetryTask {
    name: String,
    initial_delay: Duration,
    max_delay: Duration,
    max_retries: u32,
    backoff: BackoffStrategy,
    jitter: bool,
}

impl RetryTask {
    /// 创建构建器。
    #[must_use]
    pub fn builder(name: &str) -> RetryTaskBuilder {
        RetryTaskBuilder {
            name: name.to_string(),
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            max_retries: 3,
            backoff: BackoffStrategy::Exponential { multiplier: 2.0 },
            jitter: true,
        }
    }

    /// Spawn 重试任务。
    ///
    /// 闭包接收当前尝试次数（从 0 开始），返回 `Ok(())` 表示成功（停止重试），
    /// `Err(())` 表示失败（需要重试）。
    pub async fn spawn<F, Fut, T>(&self, supervisor: &TaskSupervisor, f: F)
    where
        F: Fn(u32) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, ()>> + Send + 'static,
        T: Send + 'static,
    {
        let name = self.name.clone();
        let initial = self.initial_delay;
        let max = self.max_delay;
        let max_retries = self.max_retries;
        let backoff = self.backoff;
        let jitter = self.jitter;
        let f = Arc::new(f);

        let spawn_name = name.clone();
        supervisor
            .spawn_raw(&spawn_name, move |cancel: CancellationToken| {
                let f = Arc::clone(&f);
                let log_name = name;
                async move {
                    for attempt in 0u32.. {
                        // max_retries=0 表示无限重试；>0 时检查上限
                        if max_retries > 0 && attempt >= max_retries {
                            error!(
                                task = %log_name,
                                max_retries = max_retries,
                                "retry task exhausted all attempts"
                            );
                            return;
                        }
                        // 执行任务（带 panic 隔离）
                        let f_clone = Arc::clone(&f);
                        let handle = tokio::spawn(f_clone(attempt));
                        match handle.await {
                            Ok(Ok(_)) => {
                                tracing::debug!(
                                    task = %log_name,
                                    attempt = attempt,
                                    "retry task succeeded"
                                );
                                return; // 成功，停止重试
                            }
                            Ok(Err(_)) => {
                                tracing::warn!(
                                    task = %log_name,
                                    attempt = attempt,
                                    "retry task failed, backing off"
                                );
                            }
                            Err(join_err) if join_err.is_panic() => {
                                error!(
                                    task = %log_name,
                                    attempt = attempt,
                                    "retry task panic recovered, backing off"
                                );
                            }
                            Err(_) => {
                                error!(
                                    task = %log_name,
                                    attempt = attempt,
                                    "retry task cancelled externally, backing off"
                                );
                            }
                        }

                        // 基于策略计算退避延迟
                        let delay = compute_delay(initial, attempt, max, backoff, jitter);

                        // 可中断的退避 sleep
                        tokio::select! {
                            _ = cancel.cancelled() => {
                                tracing::debug!(task = %log_name, "retry task cancelled during backoff");
                                return;
                            }
                            _ = tokio::time::sleep(delay) => {}
                        }
                    }
                }
            })
            .await;
    }
}

/// 基于策略和尝试次数计算退避延迟（含 jitter）。
///
/// - Exponential: `delay = initial * multiplier^attempt`
/// - Linear: `delay = initial * (attempt + 1)`
///
/// 均封顶于 `max_delay`。
fn compute_delay(
    initial: Duration,
    attempt: u32,
    max: Duration,
    backoff: BackoffStrategy,
    jitter: bool,
) -> Duration {
    let base = match backoff {
        BackoffStrategy::Exponential { multiplier } => {
            let exp = multiplier.powi(attempt as i32);
            Duration::from_secs_f64(initial.as_secs_f64() * exp)
        }
        BackoffStrategy::Linear => initial * (attempt + 1),
    };
    let capped = base.min(max);
    if !jitter {
        return capped;
    }
    let jitter_factor = rand::random::<f64>() * 0.2; // 0~20%
    let jittered = capped.as_secs_f64() * (1.0 + jitter_factor);
    Duration::from_secs_f64(jittered).min(max)
}

/// 构建器。
pub struct RetryTaskBuilder {
    name: String,
    initial_delay: Duration,
    max_delay: Duration,
    max_retries: u32,
    backoff: BackoffStrategy,
    jitter: bool,
}

impl RetryTaskBuilder {
    /// 初始退避延迟。默认 1 秒。
    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// 最大退避延迟封顶。默认 60 秒。
    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// 最大重试次数。默认 3 次。设为 0 表示无限重试。
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
    /// 退避策略。默认指数退避（×2）。
    pub fn backoff(mut self, strategy: BackoffStrategy) -> Self {
        self.backoff = strategy;
        self
    }

    /// 是否启用随机抖动（±20%）。默认 true。
    pub fn jitter(mut self, enabled: bool) -> Self {
        self.jitter = enabled;
        self
    }

    /// 构建并 spawn 重试任务。
    pub async fn spawn<F, Fut, T>(self, supervisor: &TaskSupervisor, f: F)
    where
        F: Fn(u32) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, ()>> + Send + 'static,
        T: Send + 'static,
    {
        let task = RetryTask {
            name: self.name,
            initial_delay: self.initial_delay,
            max_delay: self.max_delay,
            max_retries: self.max_retries,
            backoff: self.backoff,
            jitter: self.jitter,
        };
        task.spawn(supervisor, f).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_succeeds_first_attempt() {
        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel);

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        RetryTask::builder("test_ok")
            .max_retries(5)
            .initial_delay(Duration::from_millis(10))
            .spawn(&supervisor, move |_| {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    Result::<(), ()>::Ok(())
                }
            })
            .await;

        // Wait for task to complete
        tokio::time::sleep(Duration::from_millis(50)).await;
        supervisor.shutdown().await;

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel);

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        RetryTask::builder("test_fail")
            .max_retries(3)
            .initial_delay(Duration::from_millis(10))
            .max_delay(Duration::from_millis(50))
            .jitter(false)
            .spawn(&supervisor, move |_| {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    Result::<(), ()>::Err(())
                }
            })
            .await;

        // Wait for all retries to exhaust
        tokio::time::sleep(Duration::from_millis(200)).await;
        supervisor.shutdown().await;

        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_retry_cancel_interrupts_sleep() {
        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel);

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        RetryTask::builder("test_cancel")
            .max_retries(100)
            .initial_delay(Duration::from_secs(60))
            .max_delay(Duration::from_secs(60))
            .jitter(false)
            .spawn(&supervisor, move |_| {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    Result::<(), ()>::Err(())
                }
            })
            .await;

        // First attempt executes immediately
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Cancel should interrupt the 60s backoff
        let start = std::time::Instant::now();
        supervisor.shutdown().await;
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(2),
            "shutdown should be fast, took {elapsed:?}"
        );
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let cancel = CancellationToken::root();
        let supervisor = TaskSupervisor::new(cancel);

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        RetryTask::builder("test_retry_then_ok")
            .max_retries(5)
            .initial_delay(Duration::from_millis(10))
            .max_delay(Duration::from_millis(30))
            .jitter(false)
            .spawn(&supervisor, move |_attempt| {
                let c = Arc::clone(&counter_clone);
                async move {
                    let n = c.fetch_add(1, Ordering::Relaxed);
                    if n < 2 {
                        Err(())
                    } else {
                        Ok(())
                    }
                }
            })
            .await;

        // Wait for task to complete
        tokio::time::sleep(Duration::from_millis(200)).await;
        supervisor.shutdown().await;

        // Should have attempted 3 times: 2 failures + 1 success
        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }
}
