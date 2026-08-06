use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::error;

use crate::Stop;

const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

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
        self.stop.cancel();
    }
}
