use std::future::Future;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::error;

use crate::CancellationToken;

struct SupervisedHandle {
    name: String,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl SupervisedHandle {
    async fn abort_with_timeout(self, timeout: Duration) {
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

pub struct TaskSupervisor {
    cancel: CancellationToken,
    tasks: Mutex<Vec<SupervisedHandle>>,
    shutdown_timeout: Duration,
}

impl TaskSupervisor {
    #[must_use]
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
            tasks: Mutex::new(Vec::new()),
            shutdown_timeout: Duration::from_secs(5),
        }
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    #[must_use]
    pub fn child_token(&self) -> CancellationToken {
        self.cancel.child_token()
    }

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

    pub async fn shutdown(&self) {
        self.cancel.cancel();
        let tasks: Vec<SupervisedHandle> = self.tasks.lock().await.drain(..).collect();
        let timeout = self.shutdown_timeout;
        let mut join_set = tokio::task::JoinSet::new();
        for task in tasks {
            join_set.spawn(task.abort_with_timeout(timeout));
        }
        while join_set.join_next().await.is_some() {}
    }

    pub async fn task_count(&self) -> usize {
        self.tasks.lock().await.len()
    }
}

impl Drop for TaskSupervisor {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_spawn_raw_and_shutdown() {
        let supervisor = TaskSupervisor::new(CancellationToken::root());

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        supervisor
            .spawn_raw("test_task", |cancel| async move {
                tokio::select! {
                    _ = cancel.cancelled() => {}
                    _ = async {
                        tokio::time::sleep(Duration::from_secs(60)).await;
                        flag_clone.store(true, Ordering::Relaxed);
                    } => {}
                }
            })
            .await;

        assert_eq!(supervisor.task_count().await, 1);
        supervisor.shutdown().await;
        assert_eq!(supervisor.task_count().await, 0);
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_cancel_method() {
        let supervisor = TaskSupervisor::new(CancellationToken::root());

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        supervisor
            .spawn_raw("cancel_test", |cancel| async move {
                tokio::select! {
                    _ = cancel.cancelled() => {}
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {
                        flag_clone.store(true, Ordering::Relaxed);
                    }
                }
            })
            .await;

        supervisor.cancel();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_drop_triggers_cancel() {
        let parent = CancellationToken::root();
        let supervisor = TaskSupervisor::new(parent.child_token());

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        supervisor
            .spawn_raw("drop_test", |cancel| async move {
                tokio::select! {
                    _ = cancel.cancelled() => {}
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {
                        flag_clone.store(true, Ordering::Relaxed);
                    }
                }
            })
            .await;

        drop(supervisor);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!flag.load(Ordering::Relaxed));
        assert!(!parent.is_cancelled());
    }

    #[tokio::test]
    async fn test_child_token_propagates() {
        let parent = CancellationToken::root();
        let supervisor = TaskSupervisor::new(parent.child_token());

        let child = supervisor.child_token();
        assert!(!child.is_cancelled());

        supervisor.cancel();
        assert!(child.is_cancelled());
    }
}
