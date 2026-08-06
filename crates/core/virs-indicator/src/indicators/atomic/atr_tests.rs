use super::atr::{atr, atr_at};
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn atr_positive_for_sufficient_data() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let val = atr_at(&klines, last, 14).unwrap();
    assert!(val > 0.0, "ATR must be positive");
}

#[test]
fn atr_series_correct_length() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let series = atr(&klines, 14).unwrap();
    assert_eq!(series.len(), klines.len());
}

#[test]
fn atr_zero_for_zero_range_klines() {
    let klines: Vec<_> = (0..30).map(|_| kline(100.0, 100.0, 100.0, 100.0, 1000.0)).collect();
    let last = klines.len() - 1;
    let val = atr_at(&klines, last, 14).unwrap();
    assert!(val.abs() < 0.001, "ATR should be ~0 for zero-range klines, got {val}");
}

#[test]
fn atr_errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(atr_at(&klines, 9, 14).is_err());
}
