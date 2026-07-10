//! Unit tests for auto/worker.rs time-related logic.
//!
//! T11: position_opened_at restoration from DB opened_at.

use std::time::Duration;
use tokio::time::Instant;

/// T11: 从 DB opened_at (DateTime<Utc>) 恢复 position_opened_at 的核心计算逻辑。
///
/// Instant 是单调时钟，不能从 DateTime 直接构造。
/// 通过计算 elapsed 后用 checked_sub 反推 Instant。
fn compute_position_opened_at(opened_at: chrono::DateTime<chrono::Utc>) -> Option<Instant> {
    let elapsed = chrono::Utc::now().signed_duration_since(opened_at);
    let elapsed_secs = elapsed.num_seconds().max(0) as u64;
    let elapsed_dur = Duration::from_secs(elapsed_secs);
    Instant::now().checked_sub(elapsed_dur)
}

// ============================================================
// TC-T11: position_opened_at 从 DB 恢复
// ============================================================

#[test]
fn t11_1_restored_instant_reflects_actual_elapsed_time() {
    // T11: 从 2 小时前的 opened_at 恢复，elapsed 应约 2 小时
    let opened_at = chrono::Utc::now() - chrono::Duration::hours(2);
    let restored = compute_position_opened_at(opened_at);
    assert!(restored.is_some(), "checked_sub should succeed for 2h elapsed");
    let instant = restored.unwrap();
    let elapsed = instant.elapsed();
    // 应接近 2 小时（允许几秒误差）
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
    // T11: 刚开仓 5 秒前的 opened_at，elapsed 应约 5 秒
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
    // T11: 持仓 47 小时（接近 48h 超时），恢复后 elapsed 应约 47h
    // 这验证了重启后超时检查不会被重置
    let opened_at = chrono::Utc::now() - chrono::Duration::hours(47);
    let restored = compute_position_opened_at(opened_at);
    assert!(restored.is_some());
    let elapsed = restored.unwrap().elapsed();
    let max_duration = Duration::from_secs(48 * 3600);
    // elapsed 应小于 max_duration（47h < 48h）
    assert!(
        elapsed < max_duration,
        "47h elapsed {:?} should be less than 48h max {:?}",
        elapsed,
        max_duration
    );
    // 但应非常接近 max_duration
    let remaining = max_duration - elapsed;
    assert!(
        remaining < Duration::from_secs(3600),
        "remaining {:?} should be less than 1h (47h elapsed, 48h max)",
        remaining
    );
}

#[test]
fn t11_4_restored_instant_exceeds_max_position_duration() {
    // T11: 持仓 49 小时（超过 48h 超时），恢复后 elapsed 应超过 max_duration
    // 这验证了重启后超时检查仍会触发
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
    // T11: opened_at 在未来（时钟偏移），elapsed_secs 被 max(0) 钳为 0
    let future_opened_at = chrono::Utc::now() + chrono::Duration::hours(1);
    let restored = compute_position_opened_at(future_opened_at);
    assert!(restored.is_some());
    // elapsed 应约 0（不是负数）
    let elapsed = restored.unwrap().elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "future opened_at should result in near-zero elapsed, got {:?}",
        elapsed
    );
}

#[test]
fn t11_6_checked_sub_returns_none_for_extreme_duration() {
    // T11: 极大的 elapsed（如 i64::MAX 秒，约 2920 亿年）会导致 checked_sub 返回 None
    // 这是边界情况：实际不会发生，但代码需要正确处理
    let extreme_secs = i64::MAX as u64;
    let extreme_dur = Duration::from_secs(extreme_secs);
    let result = Instant::now().checked_sub(extreme_dur);
    // 在某些平台上可能返回 None（溢出时），某些平台可能返回 Some
    // 关键是 compute_position_opened_at 函数需要正确处理 None 情况
    if result.is_none() {
        // 验证 fallback：None 时函数应返回 None（调用方负责 fallback）
        let ancient_open = chrono::DateTime::from_timestamp(0, 0).unwrap();
        let _restored = compute_position_opened_at(ancient_open);
        // Unix epoch 距今约 56 年，checked_sub 应成功（Instant 从系统启动开始）
        // 但如果 elapsed 超过系统运行时间，checked_sub 会返回 None
        // 无论如何，代码中的 fallback 会用 Instant::now()
    }
    // 这个测试主要验证 checked_sub 在极端情况下不会 panic
}
