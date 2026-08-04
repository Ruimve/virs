use super::macd::{macd_at, macd_signal_at};
use crate::indicators::test_utils::{uptrend_klines, downtrend_klines};

#[test]
fn macd_positive_in_uptrend() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let val = macd_at(&klines, last, 12, 26).unwrap();
    assert!(val > 0.0, "MACD should be positive in uptrend, got {val}");
}

#[test]
fn macd_negative_in_downtrend() {
    let klines = downtrend_klines(60, 200.0, 1.0);
    let last = klines.len() - 1;
    let val = macd_at(&klines, last, 12, 26).unwrap();
    assert!(val < 0.0, "MACD should be negative in downtrend, got {val}");
}

#[test]
fn macd_signal_exists_for_sufficient_data() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let sig = macd_signal_at(&klines, last, 12, 26, 9).unwrap();
    assert!(sig.is_finite(), "Signal should be finite");
}

#[test]
fn macd_errors_on_insufficient_data() {
    let klines = uptrend_klines(20, 100.0, 1.0);
    assert!(macd_at(&klines, 19, 12, 26).is_err());
}

#[test]
fn macd_signal_errors_on_insufficient_data() {
    let klines = uptrend_klines(30, 100.0, 1.0);
    assert!(macd_signal_at(&klines, 29, 12, 26, 9).is_err());
}
