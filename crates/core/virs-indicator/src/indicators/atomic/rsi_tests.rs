use super::rsi::rsi_at;
use crate::indicators::test_utils::{uptrend_klines, downtrend_klines, sideways_klines};

#[test]
fn rsi_above_50_in_uptrend() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let rsi = rsi_at(&klines, last, 14).unwrap();
    assert!(rsi > 50.0, "RSI should be > 50 in uptrend, got {rsi}");
}

#[test]
fn rsi_below_50_in_downtrend() {
    let klines = downtrend_klines(60, 200.0, 1.0);
    let last = klines.len() - 1;
    let rsi = rsi_at(&klines, last, 14).unwrap();
    assert!(rsi < 50.0, "RSI should be < 50 in downtrend, got {rsi}");
}

#[test]
fn rsi_near_50_in_sideways() {
    let klines = sideways_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let rsi = rsi_at(&klines, last, 14).unwrap();
    assert!((rsi - 50.0).abs() < 20.0, "RSI should be near 50 in sideways, got {rsi}");
}

#[test]
fn rsi_errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(rsi_at(&klines, 9, 14).is_err());
}
