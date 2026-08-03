use std::future::Future;
use std::time::Duration;

use tracing::error;

use crate::{CancellationToken, TaskHandle};

pub fn spawn<F, Fut>(name: &str, f: F) -> TaskHandle
where
    F: FnOnce(CancellationToken) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    let _name = name.to_string();
    let handle = tokio::spawn(f(cancel_clone));
    TaskHandle::new(cancel, handle)
}

pub fn spawn_periodic<F, Fut>(
    name: &str,
    interval: Duration,
    first_tick_immediate: bool,
    f: F,
) -> TaskHandle
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    let f = std::sync::Arc::new(f);
    let log_name = name.to_string();

    let handle = tokio::spawn(async move {
        let mut tick = if first_tick_immediate {
            tokio::time::interval(interval)
        } else {
            let start = tokio::time::Instant::now() + interval;
            tokio::time::interval_at(start, interval)
        };

        loop {
            tokio::select! {
                _ = cancel_clone.cancelled() => break,
                _ = tick.tick() => {
                    let f = std::sync::Arc::clone(&f);
                    let inner_handle = tokio::spawn(f());
                    if let Err(join_err) = inner_handle.await {
                        if join_err.is_panic() {
                            error!(task = %log_name, "periodic task panic recovered");
                        }
                    }
                }
            }
        }
    });

    TaskHandle::new(cancel, handle)
}
