use std::future::Future;
use std::time::Duration;

use tracing::Instrument;

use crate::{Stop, TaskHandle};

/*
 * 任务派生入口：工作区唯一的异步任务派生方式，禁止直接使用 tokio::spawn。
 * 传入的闭包接收 Stop 参数，可在任务内部监听取消信号。
 */
pub fn spawn<F, Fut>(name: &str, f: F) -> TaskHandle
where
    F: FnOnce(Stop) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let stop = Stop::new();
    let stop_clone = stop.clone();
    let span = tracing::info_span!("task", name = name);
    let handle = tokio::spawn(f(stop_clone).instrument(span));
    TaskHandle::new(stop, handle)
}

/*
 * 周期性任务派生：按固定间隔重复执行闭包，支持首次立即触发或延迟触发。
 * 通过 select! 监听取消信号，每次执行在独立子任务中运行以防止单次 panic 影响后续周期。
 */
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
    let stop = Stop::new();
    let stop_clone = stop.clone();
    let f = std::sync::Arc::new(f);
    let log_name = name.to_string();

    let handle = tokio::spawn(async move {
        /* 根据标志决定首次触发是立即还是延迟一个周期 */
        let mut tick = if first_tick_immediate {
            tokio::time::interval(interval)
        } else {
            let start = tokio::time::Instant::now() + interval;
            tokio::time::interval_at(start, interval)
        };

        loop {
            tokio::select! {
                _ = stop_clone.cancelled() => break,
                _ = tick.tick() => {
                    /* 在独立子任务中执行，防止单次 panic 导致整个周期循环终止 */
                    let f = std::sync::Arc::clone(&f);
                    let inner_handle = tokio::spawn(f());
                    if let Err(join_err) = inner_handle.await {
                        if join_err.is_panic() {
                            tracing::error!(task = %log_name, "periodic task panic recovered");
                        }
                    }
                }
            }
        }
    });

    TaskHandle::new(stop, handle)
}
