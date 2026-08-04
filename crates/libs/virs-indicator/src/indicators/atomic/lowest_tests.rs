use super::lowest::lowest_at;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn lowest_returns_min_low() {
    let klines = uptrend_klines(30, 100.0, 1.0);
    let last = klines.len() - 1;
    let val = lowest_at(&klines, last, 20).unwrap();
    let expected = klines[last - 19..=last].iter().map(|k| k.low).fold(f64::MAX, f64::min);
    assert!((val - expected).abs() < 0.001, "Lowest should be {expected}, got {val}");
}

#[test]
fn lowest_of_constant_is_constant() {
    let klines: Vec<_> = (0..30).map(|_| kline(100.0, 105.0, 95.0, 100.0, 1000.0)).collect();
    let val = lowest_at(&klines, 29, 20).unwrap();
    assert!((val - 95.0).abs() < 0.001, "Lowest of constant lows should be 95.0, got {val}");
}

#[test]
fn lowest_errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(lowest_at(&klines, 9, 20).is_err());
}
