use super::bbands_width::compute;
use crate::indicators::test_utils::{uptrend_klines, sideways_klines};

#[test]
fn width_positive_for_sufficient_data() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let val = compute(&klines, last, 20, 2).unwrap();
    assert!(val > 0.0, "Band width must be positive, got {val}");
}

#[test]
fn width_smaller_in_sideways_than_trend() {
    let trend_klines = uptrend_klines(60, 100.0, 2.0);
    let sideways = sideways_klines(60, 100.0, 0.5);
    let trend_width = compute(&trend_klines, 59, 20, 2).unwrap();
    let sideways_width = compute(&sideways, 59, 20, 2).unwrap();
    assert!(sideways_width < trend_width, "Sideways width ({sideways_width}) should be < trend width ({trend_width})");
}

#[test]
fn width_errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(compute(&klines, 9, 20, 2).is_err());
}
