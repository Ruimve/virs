use super::ema::ema_at;
use crate::indicators::test_utils::uptrend_klines;

#[test]
fn ema_returns_value_for_sufficient_data() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let val = ema_at(&klines, last, 20).unwrap();
    assert!(val > 100.0, "EMA on uptrend should be > start price");
}

#[test]
fn ema_lagging_behind_price_in_uptrend() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let ema = ema_at(&klines, last, 20).unwrap();
    let price = klines[last].close;
    assert!(ema < price, "EMA should lag behind price in uptrend");
}

#[test]
fn ema_shorter_period_closer_to_price() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let last = klines.len() - 1;
    let ema20 = ema_at(&klines, last, 20).unwrap();
    let ema50 = ema_at(&klines, last, 50).unwrap();
    let price = klines[last].close;
    assert!((price - ema20).abs() < (price - ema50).abs(), "Shorter EMA should be closer to price");
}

#[test]
fn ema_errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(ema_at(&klines, 9, 20).is_err());
}

#[test]
fn ema_errors_on_empty_klines() {
    let klines: Vec<virs_type::Kline> = vec![];
    assert!(ema_at(&klines, 0, 20).is_err());
}
