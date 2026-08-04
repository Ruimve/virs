use super::current_price::compute;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn returns_last_close() {
    let klines = uptrend_klines(30, 100.0, 1.0);
    let val = compute(&klines).unwrap();
    assert_eq!(val, klines.last().unwrap().close);
}

#[test]
fn returns_single_kline_close() {
    let klines = vec![kline(100.0, 105.0, 95.0, 103.0, 1000.0)];
    let val = compute(&klines).unwrap();
    assert_eq!(val, 103.0);
}

#[test]
fn errors_on_empty_klines() {
    let klines: Vec<virs_types::Kline> = vec![];
    assert!(compute(&klines).is_err());
}
