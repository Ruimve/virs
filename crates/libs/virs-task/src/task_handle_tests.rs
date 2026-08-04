use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_drop_cancels() {
    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&flag);

    let handle = spawn("test", |stop| async move {
        tokio::select! {
            _ = stop.cancelled() => {}
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                flag_clone.store(true, Ordering::Relaxed);
            }
        }
    });

    drop(handle);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!flag.load(Ordering::Relaxed));
}

#[tokio::test]
async fn test_join_with_timeout() {
    let handle = spawn("test", |stop| async move {
        tokio::select! {
            _ = stop.cancelled() => {}
            _ = tokio::time::sleep(Duration::from_secs(60)) => {}
        }
    });

    let start = std::time::Instant::now();
    handle.join_with_timeout(Duration::from_secs(2)).await;
    assert!(start.elapsed() < Duration::from_secs(3));
}
