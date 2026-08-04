use super::ema_cross_bars::compute;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn returns_minus_one_for_no_cross_in_uptrend() {
    // 持续上涨：EMA20 始终在 EMA50 之上，不会交叉
    let klines = uptrend_klines(100, 100.0, 1.0);
    let val = compute(&klines, 20, 50).unwrap();
    assert_eq!(val, -1, "Should return -1 (no recent cross) in steady uptrend");
}

#[test]
fn returns_non_negative_when_cross_exists() {
    // 50 根平盘建立 EMA 历史 → 10 根下跌让 EMA20 跌破 EMA50 → 15 根上涨让 EMA20 交叉回升
    // 交叉发生在最近 20 根内
    let mut klines: Vec<_> = (0..50).map(|_| kline(100.0, 101.0, 99.0, 100.0, 1000.0)).collect();
    klines.extend((0..10).map(|i| kline(100.0 + i as f64, 101.0, 99.0, 100.0 - i as f64, 1000.0)));
    klines.extend((0..15).map(|i| kline(90.0 + i as f64 * 2.0, 92.0 + i as f64 * 2.0, 88.0, 90.0 + i as f64 * 2.0, 1000.0)));
    let val = compute(&klines, 20, 50).unwrap();
    assert!(val >= 0, "Should find a cross (>= 0), got {val}");
}

#[test]
fn errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(compute(&klines, 20, 50).is_err());
}
