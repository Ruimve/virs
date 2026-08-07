use super::bars_outside::compute;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn zero_when_price_inside_band() {

    let klines: Vec<_> = (0..30).map(|i| kline(100.0, 101.0, 99.0, 100.0, 1000.0)).collect();
    let val = compute(&klines, 20, 2).unwrap();
    assert_eq!(val, 0, "Bars outside should be 0 when price stays inside band");
}

#[test]
fn positive_when_price_above_upper() {

    let mut klines: Vec<_> = (0..25).map(|i| kline(100.0, 101.0, 99.0, 100.0, 1000.0)).collect();

    klines.push(kline(100.0, 130.0, 100.0, 130.0, 1000.0));
    klines.push(kline(130.0, 140.0, 125.0, 140.0, 1000.0));
    klines.push(kline(140.0, 150.0, 135.0, 150.0, 1000.0));
    klines.push(kline(150.0, 160.0, 145.0, 160.0, 1000.0));
    klines.push(kline(160.0, 170.0, 155.0, 170.0, 1000.0));
    let val = compute(&klines, 20, 2).unwrap();
    assert!(val > 0, "Bars outside should be positive when price above upper, got {val}");
}

#[test]
fn errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(compute(&klines, 20, 2).is_err());
}
