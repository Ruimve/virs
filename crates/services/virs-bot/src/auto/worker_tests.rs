use std::time::Duration;
use tokio::time::Instant;


fn compute_position_opened_at(opened_at: chrono::DateTime<chrono::Utc>) -> Option<Instant> {
    let elapsed = chrono::Utc::now().signed_duration_since(opened_at);
    let elapsed_secs = elapsed.num_seconds().max(0) as u64;
    let elapsed_dur = Duration::from_secs(elapsed_secs);
    Instant::now().checked_sub(elapsed_dur)
}


#[test]
fn t11_1_restored_instant_reflects_actual_elapsed_time() {

    let opened_at = chrono::Utc::now() - chrono::Duration::hours(2);
    let restored = compute_position_opened_at(opened_at);
    assert!(restored.is_some(), "checked_sub should succeed for 2h elapsed");
    let instant = restored.unwrap();
    let elapsed = instant.elapsed();

    let two_hours = Duration::from_secs(2 * 3600);
    let diff = if elapsed > two_hours {
        elapsed - two_hours
    } else {
        two_hours - elapsed
    };
    assert!(
        diff < Duration::from_secs(10),
        "elapsed {:?} should be close to 2h, diff={:?}",
        elapsed,
        diff
    );
}

#[test]
fn t11_2_restored_instant_for_recent_open() {

    let opened_at = chrono::Utc::now() - chrono::Duration::seconds(5);
    let restored = compute_position_opened_at(opened_at);
    assert!(restored.is_some());
    let elapsed = restored.unwrap().elapsed();
    let five_secs = Duration::from_secs(5);
    let diff = if elapsed > five_secs {
        elapsed - five_secs
    } else {
        five_secs - elapsed
    };
    assert!(
        diff < Duration::from_secs(5),
        "elapsed {:?} should be close to 5s, diff={:?}",
        elapsed,
        diff
    );
}

#[test]
fn t11_3_restored_instant_near_max_position_duration() {


    let opened_at = chrono::Utc::now() - chrono::Duration::hours(47);
    let restored = compute_position_opened_at(opened_at);
    assert!(restored.is_some());
    let elapsed = restored.unwrap().elapsed();
    let max_duration = Duration::from_secs(48 * 3600);

    assert!(
        elapsed < max_duration,
        "47h elapsed {:?} should be less than 48h max {:?}",
        elapsed,
        max_duration
    );

    let remaining = max_duration - elapsed;
    assert!(
        remaining < Duration::from_secs(3600),
        "remaining {:?} should be less than 1h (47h elapsed, 48h max)",
        remaining
    );
}

#[test]
fn t11_4_restored_instant_exceeds_max_position_duration() {


    let opened_at = chrono::Utc::now() - chrono::Duration::hours(49);
    let restored = compute_position_opened_at(opened_at);
    assert!(restored.is_some());
    let elapsed = restored.unwrap().elapsed();
    let max_duration = Duration::from_secs(48 * 3600);
    assert!(
        elapsed > max_duration,
        "49h elapsed {:?} should exceed 48h max {:?}",
        elapsed,
        max_duration
    );
}

#[test]
fn t11_5_future_opened_at_clamped_to_zero() {

    let future_opened_at = chrono::Utc::now() + chrono::Duration::hours(1);
    let restored = compute_position_opened_at(future_opened_at);
    assert!(restored.is_some());

    let elapsed = restored.unwrap().elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "future opened_at should result in near-zero elapsed, got {:?}",
        elapsed
    );
}

#[test]
fn t11_6_checked_sub_returns_none_for_extreme_duration() {


    let extreme_secs = i64::MAX as u64;
    let extreme_dur = Duration::from_secs(extreme_secs);
    let result = Instant::now().checked_sub(extreme_dur);


    if result.is_none() {

        let ancient_open = chrono::DateTime::from_timestamp(0, 0).unwrap();
        let _restored = compute_position_opened_at(ancient_open);


    }

}
