use super::candle_body::compute;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn positive_for_bullish_candle() {
    let klines = vec![kline(100.0, 105.0, 95.0, 103.0, 1000.0)];
    let val = compute(&klines).unwrap();
    assert!((val - 3.0).abs() < 0.001, "Bullish body should be 3.0 (103-100), got {val}");
}

#[test]
fn negative_for_bearish_candle() {
    let klines = vec![kline(103.0, 105.0, 95.0, 100.0, 1000.0)];
    let val = compute(&klines).unwrap();
    assert!((val - (-3.0)).abs() < 0.001, "Bearish body should be -3.0 (100-103), got {val}");
}

#[test]
fn zero_for_doji() {
    let klines = vec![kline(100.0, 105.0, 95.0, 100.0, 1000.0)];
    let val = compute(&klines).unwrap();
    assert!(val.abs() < 0.001, "Doji body should be ~0, got {val}");
}

#[test]
fn errors_on_empty_klines() {
    let klines: Vec<virs_type::Kline> = vec![];
    assert!(compute(&klines).is_err());
}
