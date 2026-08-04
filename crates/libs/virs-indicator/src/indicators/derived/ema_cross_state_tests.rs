use super::ema_cross_state::compute;
use crate::indicators::test_utils::{uptrend_klines, downtrend_klines};

#[test]
fn golden_cross_in_uptrend() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let val = compute(&klines, 20, 50).unwrap();
    assert_eq!(val, "金叉(多头)", "Should be golden cross in uptrend, got {val}");
}

#[test]
fn death_cross_in_downtrend() {
    let klines = downtrend_klines(60, 200.0, 1.0);
    let val = compute(&klines, 20, 50).unwrap();
    assert_eq!(val, "死叉(空头)", "Should be death cross in downtrend, got {val}");
}

#[test]
fn errors_on_insufficient_data() {
    let klines = uptrend_klines(30, 100.0, 1.0);
    assert!(compute(&klines, 20, 50).is_err());
}
