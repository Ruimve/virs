use super::change_pct::compute;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn positive_in_uptrend() {
    let klines = uptrend_klines(30, 100.0, 1.0);
    let val = compute(&klines, 1).unwrap();
    assert!(val > 0.0, "Change pct should be positive in uptrend, got {val}");
}

#[test]
fn correct_value_for_1_period() {
    // close[29] = 129, close[28] = 128 → (129-128)/128*100
    let klines = uptrend_klines(30, 100.0, 1.0);
    let val = compute(&klines, 1).unwrap();
    let expected = (129.0 - 128.0) / 128.0 * 100.0;
    assert!((val - expected).abs() < 0.001, "Change pct should be {expected}, got {val}");
}

#[test]
fn zero_change_for_constant_price() {
    let klines: Vec<_> = (0..30).map(|_| kline(100.0, 105.0, 95.0, 100.0, 1000.0)).collect();
    let val = compute(&klines, 1).unwrap();
    assert!(val.abs() < 0.001, "Change pct should be ~0 for constant price, got {val}");
}

#[test]
fn errors_on_insufficient_data() {
    let klines = uptrend_klines(5, 100.0, 1.0);
    assert!(compute(&klines, 10).is_err());
}
