//! Unit tests for types.rs

use crate::types::*;

// ── Timeframe ──────────────────────────────────────────────

#[test]
fn t1_1_timeframe_all_count() {
    let all = Timeframe::all();
    assert_eq!(all.len(), 6);
}

#[test]
fn t1_2_timeframe_as_str() {
    assert_eq!(Timeframe::M1.as_str(), "1m");
    assert_eq!(Timeframe::M5.as_str(), "5m");
    assert_eq!(Timeframe::M15.as_str(), "15m");
    assert_eq!(Timeframe::H1.as_str(), "1h");
    assert_eq!(Timeframe::H4.as_str(), "4h");
    assert_eq!(Timeframe::D1.as_str(), "1d");
}

#[test]
fn t1_3_timeframe_ms() {
    assert_eq!(Timeframe::M1.ms(), 60_000);
    assert_eq!(Timeframe::M5.ms(), 300_000);
    assert_eq!(Timeframe::M15.ms(), 900_000);
    assert_eq!(Timeframe::H1.ms(), 3_600_000);
    assert_eq!(Timeframe::H4.ms(), 14_400_000);
    assert_eq!(Timeframe::D1.ms(), 86_400_000);
}

#[test]
fn t1_4_timeframe_minutes() {
    assert_eq!(Timeframe::M1.minutes(), 1);
    assert_eq!(Timeframe::M5.minutes(), 5);
    assert_eq!(Timeframe::M15.minutes(), 15);
    assert_eq!(Timeframe::H1.minutes(), 60);
    assert_eq!(Timeframe::H4.minutes(), 240);
    assert_eq!(Timeframe::D1.minutes(), 1440);
}

#[test]
fn t1_5_timeframe_default_limit() {
    for tf in Timeframe::all() {
        assert_eq!(tf.default_limit(), 1000);
    }
}

#[test]
fn t1_6_timeframe_from_str_lossy() {
    assert_eq!(Timeframe::from_str_lossy("1m"), Some(Timeframe::M1));
    assert_eq!(Timeframe::from_str_lossy("5m"), Some(Timeframe::M5));
    assert_eq!(Timeframe::from_str_lossy("15m"), Some(Timeframe::M15));
    assert_eq!(Timeframe::from_str_lossy("1h"), Some(Timeframe::H1));
    assert_eq!(Timeframe::from_str_lossy("4h"), Some(Timeframe::H4));
    assert_eq!(Timeframe::from_str_lossy("1d"), Some(Timeframe::D1));
    assert_eq!(Timeframe::from_str_lossy("1D"), Some(Timeframe::D1));
    assert_eq!(Timeframe::from_str_lossy("invalid"), None);
}

#[test]
fn t1_7_timeframe_display() {
    assert_eq!(format!("{}", Timeframe::M1), "1m");
    assert_eq!(format!("{}", Timeframe::H1), "1h");
    assert_eq!(format!("{}", Timeframe::D1), "1d");
}

// ── subscription_key ───────────────────────────────────────

#[test]
fn t2_1_subscription_key_basic() {
    let key = subscription_key("binance", "BTC/USDT");
    assert_eq!(key, "binance:BTC/USDT");
}

#[test]
fn t2_2_subscription_key_case() {
    let key = subscription_key("Binance", "btc/usdt");
    assert_eq!(key, "binance:BTC/USDT");
}

// ── align_open_time ────────────────────────────────────────

#[test]
fn t3_1_align_open_time_m1() {
    // 1700000000500 → aligned to 1700000000000 (divisible by 60000)
    let base = (1700000000500_i64 / 60_000) * 60_000;
    let aligned = align_open_time(1700000000500, Timeframe::M1);
    assert_eq!(aligned, base);
    assert_eq!(aligned % 60_000, 0);
}

#[test]
fn t3_2_align_open_time_h1() {
    // Align to hour boundary
    let aligned = align_open_time(1700001234567, Timeframe::H1);
    assert_eq!(aligned % 3_600_000, 0);
}

#[test]
fn t3_3_align_open_time_d1() {
    // Align to day boundary
    let aligned = align_open_time(1700001234567, Timeframe::D1);
    assert_eq!(aligned % 86_400_000, 0);
}

#[test]
fn t3_4_align_open_time_exact() {
    // Already aligned → unchanged (use a value divisible by 60000)
    let exact = 1_700_000_040_000; // 1700000040000 / 60000 = integer
    assert_eq!(exact % 60_000, 0);
    let aligned = align_open_time(exact, Timeframe::M1);
    assert_eq!(aligned, exact);
}

// ── BacktestRangeLimit ─────────────────────────────────────

#[test]
fn t4_1_backtest_range_m1() {
    let limit = BacktestRangeLimit::for_timeframe(Timeframe::M1);
    assert_eq!(limit.timeframe, Timeframe::M1);
    assert_eq!(limit.max_days, 7);
    assert_eq!(limit.recommended_days, 3);
    assert_eq!(limit.estimated_candles, 7 * 24 * 60);
}

#[test]
fn t4_2_backtest_range_d1() {
    let limit = BacktestRangeLimit::for_timeframe(Timeframe::D1);
    assert_eq!(limit.timeframe, Timeframe::D1);
    assert_eq!(limit.max_days, 1825);
    assert_eq!(limit.recommended_days, 365);
    assert_eq!(limit.estimated_candles, 1825);
}

#[test]
fn t4_3_backtest_range_all_limits() {
    let limits = BacktestRangeLimit::all_limits();
    assert_eq!(limits.len(), 6);
    // Verify ascending order by max_days
    for i in 1..limits.len() {
        assert!(limits[i].max_days >= limits[i - 1].max_days);
    }
}

#[test]
fn t4_4_backtest_range_info_from() {
    let limit = BacktestRangeLimit::for_timeframe(Timeframe::H1);
    let info: BacktestRangeInfo = limit.into();
    assert_eq!(info.timeframe, "1h");
    assert_eq!(info.max_days, 365);
    assert_eq!(info.recommended_days, 90);
}

#[test]
fn t4_5_backtest_range_estimates() {
    let limit = BacktestRangeLimit::for_timeframe(Timeframe::M5);
    assert_eq!(limit.estimated_candles, 30 * 24 * 12);
    assert_eq!(limit.estimated_1m_required, 30 * 24 * 60);
}
