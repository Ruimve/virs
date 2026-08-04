use super::atr_pct::compute;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn atr_pct_positive() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let val = compute(&klines, 14).unwrap();
    assert!(val > 0.0, "ATR pct must be positive, got {val}");
}

#[test]
fn atr_pct_near_zero_for_flat_klines() {
    let klines: Vec<_> = (0..30).map(|_| kline(100.0, 100.0, 100.0, 100.0, 1000.0)).collect();
    let val = compute(&klines, 14).unwrap();
    assert!(val.abs() < 0.01, "ATR pct should be ~0 for flat klines, got {val}");
}

#[test]
fn errors_on_insufficient_data() {
    let klines = uptrend_klines(5, 100.0, 1.0);
    assert!(compute(&klines, 14).is_err());
}
