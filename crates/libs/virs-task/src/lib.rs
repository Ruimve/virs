use std::future::Future;
use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::error;

pub use tokio_util::sync::CancellationToken;

pub struct TaskHandle {
    cancel: CancellationToken,
    handle: Option<JoinHandle<()>>,
}

impl TaskHandle {
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    pub async fn join(mut self) {
        if let Some(h) = self.handle.take() {
            if let Err(e) = h.await {
                if e.is_panic() {
                    error!("task panicked");
                }
            }
        }
    }

    pub fn abort(&mut self) {
        if let Some(h) = &self.handle {
            h.abort();
        }
    }

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
                    abort_handle.abort();
                    error!("task timed out, aborted");
                }
            }
        }
    }
}

impl Drop for TaskHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

pub fn spawn<F, Fut>(name: &str, f: F) -> TaskHandle
where
    F: FnOnce(CancellationToken) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    let _name = name.to_string();
    let handle = tokio::spawn(f(cancel_clone));
    TaskHandle {
        cancel,
        handle: Some(handle),
    }
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

    TaskHandle {
        cancel,
        handle: Some(handle),
    }
}

#[cfg(test)]
mod lib_tests;
