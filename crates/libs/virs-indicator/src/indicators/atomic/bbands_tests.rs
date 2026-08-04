use super::bbands::bbands_at;
use crate::indicators::test_utils::{uptrend_klines, sideways_klines};

#[test]
fn bbands_upper_above_middle_above_lower() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let (upper, middle, lower) = bbands_at(&klines, last, 20, 2.0).unwrap();
    assert!(upper > middle, "Upper must be > middle");
    assert!(middle > lower, "Middle must be > lower");
}

#[test]
fn bbands_middle_near_last_price_in_sideways() {
    let klines = sideways_klines(60, 100.0, 2.0);
    let last = klines.len() - 1;
    let (_, middle, _) = bbands_at(&klines, last, 20, 2.0).unwrap();
    let price = klines[last].close;
    assert!((middle - price).abs() < 5.0, "Middle should be near price in sideways, middle={middle}, price={price}");
}

#[test]
fn bbands_width_positive() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let (upper, _, lower) = bbands_at(&klines, last, 20, 2.0).unwrap();
    assert!(upper - lower > 0.0, "Band width must be positive");
}

#[test]
fn bbands_errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(bbands_at(&klines, 9, 20, 2.0).is_err());
}
