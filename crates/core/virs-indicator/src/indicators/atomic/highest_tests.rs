use super::highest::highest_at;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn highest_returns_max_high() {
    let klines = uptrend_klines(30, 100.0, 1.0);
    let last = klines.len() - 1;
    let val = highest_at(&klines, last, 20).unwrap();
    let expected = klines[last - 19..=last].iter().map(|k| k.high).fold(f64::MIN, f64::max);
    assert!((val - expected).abs() < 0.001, "Highest should be {expected}, got {val}");
}

#[test]
fn highest_of_constant_is_constant() {
    let klines: Vec<_> = (0..30).map(|_| kline(100.0, 105.0, 95.0, 100.0, 1000.0)).collect();
    let val = highest_at(&klines, 29, 20).unwrap();
    assert!((val - 105.0).abs() < 0.001, "Highest of constant highs should be 105.0, got {val}");
}

#[test]
fn highest_errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(highest_at(&klines, 9, 20).is_err());
}
