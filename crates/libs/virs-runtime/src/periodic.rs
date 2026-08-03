use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tracing::error;

use crate::{CancellationToken, TaskSupervisor};

pub struct PeriodicTask;

impl PeriodicTask {
    pub async fn spawn<F, Fut>(
        name: &str,
        interval: Duration,
        first_tick_immediate: bool,
        extra_cancel: Option<CancellationToken>,
        supervisor: &TaskSupervisor,
        f: F,
    ) where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let f = Arc::new(f);
        let log_name = name.to_string();

        supervisor
            .spawn_raw(name, move |cancel: CancellationToken| {
                let f = Arc::clone(&f);
                let extra_cancel = extra_cancel.clone();
                let log_name = log_name.clone();
                async move {
                    let mut tick = if first_tick_immediate {
                        tokio::time::interval(interval)
                    } else {
                        let start = tokio::time::Instant::now() + interval;
                        tokio::time::interval_at(start, interval)
                    };

                    loop {
                        match &extra_cancel {
                            Some(ec) => {
                                tokio::select! {
                                    _ = cancel.cancelled() => break,
                                    _ = ec.cancelled() => break,
                                    _ = tick.tick() => {
                                        let f = Arc::clone(&f);
                                        let handle = tokio::spawn(f());
                                        if let Err(join_err) = handle.await {
                                            if join_err.is_panic() {
                                                error!(task = %log_name, "periodic task panic recovered");
                                            }
                                        }
                                    }
                                }
                            }
                            None => {
                                tokio::select! {
                                    _ = cancel.cancelled() => break,
                                    _ = tick.tick() => {
                                        let f = Arc::clone(&f);
                                        let handle = tokio::spawn(f());
                                        if let Err(join_err) = handle.await {
                                            if join_err.is_panic() {
                                                error!(task = %log_name, "periodic task panic recovered");
                                            }
                                        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_first_tick_immediate() {
        let supervisor = TaskSupervisor::new(CancellationToken::root());
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        PeriodicTask::spawn(
            "test_immediate",
            Duration::from_millis(50),
            true,
            None,
            &supervisor,
            move || {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                }
            },
        )
        .await;

        tokio::time::sleep(Duration::from_millis(120)).await;
        supervisor.shutdown().await;
        assert!(counter.load(Ordering::Relaxed) >= 2);
    }

    #[tokio::test]
    async fn test_first_tick_delayed() {
        let supervisor = TaskSupervisor::new(CancellationToken::root());
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        PeriodicTask::spawn(
            "test_delayed",
            Duration::from_millis(100),
            false,
            None,
            &supervisor,
            move || {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                }
            },
        )
        .await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::Relaxed), 0);

        tokio::time::sleep(Duration::from_millis(120)).await;
        supervisor.shutdown().await;
        assert!(counter.load(Ordering::Relaxed) >= 1);
    }

    #[tokio::test]
    async fn test_cancel_interrupts() {
        let supervisor = TaskSupervisor::new(CancellationToken::root());
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        PeriodicTask::spawn(
            "test_cancel",
            Duration::from_secs(60),
            true,
            None,
            &supervisor,
            move || {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                }
            },
        )
        .await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        let start = std::time::Instant::now();
        supervisor.shutdown().await;
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn test_extra_cancel() {
        let supervisor = TaskSupervisor::new(CancellationToken::root());
        let extra = CancellationToken::root();
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        PeriodicTask::spawn(
            "test_extra",
            Duration::from_millis(50),
            true,
            Some(extra.clone()),
            &supervisor,
            move || {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                }
            },
        )
        .await;

        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(counter.load(Ordering::Relaxed) >= 1);

        extra.cancel();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let count_after = counter.load(Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(counter.load(Ordering::Relaxed), count_after);

        supervisor.shutdown().await;
    }
}
