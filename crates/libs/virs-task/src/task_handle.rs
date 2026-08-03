use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::error;

use crate::CancellationToken;

pub struct TaskHandle {
    cancel: CancellationToken,
    handle: Option<JoinHandle<()>>,
}

impl TaskHandle {
    pub(crate) fn new(cancel: CancellationToken, handle: JoinHandle<()>) -> Self {
        TaskHandle {
            cancel,
            handle: Some(handle),
        }
    }

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
