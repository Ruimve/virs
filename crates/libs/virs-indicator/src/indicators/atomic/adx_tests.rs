use super::adx::adx_at;
use crate::indicators::test_utils::{uptrend_klines, sideways_klines};

#[test]
fn adx_higher_in_trend_than_sideways() {
    let trend_klines = uptrend_klines(60, 100.0, 2.0);
    let sideways_klines = sideways_klines(60, 100.0, 0.5);
    let trend_adx = adx_at(&trend_klines, 59, 14).unwrap();
    let sideways_adx = adx_at(&sideways_klines, 59, 14).unwrap();
    assert!(trend_adx > sideways_adx, "ADX should be higher in trend ({trend_adx}) than sideways ({sideways_adx})");
}

#[test]
fn adx_positive() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let val = adx_at(&klines, 59, 14).unwrap();
    assert!(val > 0.0, "ADX must be positive, got {val}");
}

#[test]
fn adx_errors_on_insufficient_data() {
    let klines = uptrend_klines(20, 100.0, 1.0);
    assert!(adx_at(&klines, 19, 14).is_err());
}
