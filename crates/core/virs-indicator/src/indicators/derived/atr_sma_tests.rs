use super::atr_sma::compute;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn atr_sma_positive() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let val = compute(&klines, 14, 20).unwrap();
    assert!(val > 0.0, "ATR SMA must be positive, got {val}");
}

#[test]
fn atr_sma_near_zero_for_flat_klines() {
    let klines: Vec<_> = (0..40).map(|_| kline(100.0, 100.0, 100.0, 100.0, 1000.0)).collect();
    let val = compute(&klines, 14, 20).unwrap();
    assert!(val.abs() < 0.01, "ATR SMA should be ~0 for flat klines, got {val}");
}

#[test]
fn errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(compute(&klines, 14, 20).is_err());
}
