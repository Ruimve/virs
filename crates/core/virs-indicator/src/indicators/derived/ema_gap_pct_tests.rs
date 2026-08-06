use super::ema_gap_pct::compute;
use crate::indicators::test_utils::{uptrend_klines, downtrend_klines};

#[test]
fn positive_in_uptrend() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let val = compute(&klines, 20, 50).unwrap();
    assert!(val > 0.0, "EMA gap pct should be positive in uptrend (EMA20 > EMA50), got {val}");
}

#[test]
fn negative_in_downtrend() {
    let klines = downtrend_klines(60, 200.0, 1.0);
    let val = compute(&klines, 20, 50).unwrap();
    assert!(val < 0.0, "EMA gap pct should be negative in downtrend (EMA20 < EMA50), got {val}");
}

#[test]
fn errors_on_insufficient_data() {
    let klines = uptrend_klines(30, 100.0, 1.0);
    assert!(compute(&klines, 20, 50).is_err());
}
