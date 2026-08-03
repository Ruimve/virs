use super::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_spawn_cancel() {
    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&flag);

    let handle = spawn("test", |cancel| async move {
        tokio::select! {
            _ = cancel.cancelled() => {}
            _ = async {
                tokio::time::sleep(Duration::from_secs(60)).await;
                flag_clone.store(true, Ordering::Relaxed);
            } => {}
        }
    });

    handle.cancel();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!flag.load(Ordering::Relaxed));
}

#[tokio::test]
async fn test_independent_tasks() {
    let flag_a = Arc::new(AtomicBool::new(false));
    let flag_b = Arc::new(AtomicBool::new(false));
    let flag_a_clone = Arc::clone(&flag_a);
    let flag_b_clone = Arc::clone(&flag_b);

    let handle_a = spawn("a", |cancel| async move {
        tokio::select! {
            _ = cancel.cancelled() => {}
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                flag_a_clone.store(true, Ordering::Relaxed);
            }
        }
    });

    let handle_b = spawn("b", |cancel| async move {
        tokio::select! {
            _ = cancel.cancelled() => {}
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                flag_b_clone.store(true, Ordering::Relaxed);
            }
        }
    });

    handle_a.cancel();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(!flag_a.load(Ordering::Relaxed));
    assert!(!flag_b.load(Ordering::Relaxed));

    handle_b.cancel();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!flag_b.load(Ordering::Relaxed));
}

#[tokio::test]
async fn test_periodic_first_tick_immediate() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = spawn_periodic("test", Duration::from_millis(50), true, move || {
        let c = Arc::clone(&counter_clone);
        async move {
            c.fetch_add(1, Ordering::Relaxed);
        }
    });

    tokio::time::sleep(Duration::from_millis(120)).await;
    handle.cancel();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(counter.load(Ordering::Relaxed) >= 2);
}

#[tokio::test]
async fn test_periodic_first_tick_delayed() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = spawn_periodic("test", Duration::from_millis(100), false, move || {
        let c = Arc::clone(&counter_clone);
        async move {
            c.fetch_add(1, Ordering::Relaxed);
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(counter.load(Ordering::Relaxed), 0);

    tokio::time::sleep(Duration::from_millis(120)).await;
    handle.cancel();
    assert!(counter.load(Ordering::Relaxed) >= 1);
}

#[tokio::test]
async fn test_periodic_cancel_stops() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = spawn_periodic("test", Duration::from_millis(50), true, move || {
        let c = Arc::clone(&counter_clone);
        async move {
            c.fetch_add(1, Ordering::Relaxed);
        }
    });

    tokio::time::sleep(Duration::from_millis(120)).await;
    let count = counter.load(Ordering::Relaxed);
    assert!(count >= 2);

    handle.cancel();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(counter.load(Ordering::Relaxed), count);
}

#[tokio::test]
async fn test_periodic_independent() {
    let counter_a = Arc::new(AtomicU32::new(0));
    let counter_b = Arc::new(AtomicU32::new(0));
    let counter_a_clone = Arc::clone(&counter_a);
    let counter_b_clone = Arc::clone(&counter_b);

    let handle_a = spawn_periodic("a", Duration::from_millis(50), true, move || {
        let c = Arc::clone(&counter_a_clone);
        async move {
            c.fetch_add(1, Ordering::Relaxed);
        }
    });

    let handle_b = spawn_periodic("b", Duration::from_millis(50), true, move || {
        let c = Arc::clone(&counter_b_clone);
        async move {
            c.fetch_add(1, Ordering::Relaxed);
        }
    });

    tokio::time::sleep(Duration::from_millis(120)).await;
    handle_a.cancel();

    let count_b_before = counter_b.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(120)).await;

    assert!(counter_b.load(Ordering::Relaxed) > count_b_before);

    handle_b.cancel();
}
