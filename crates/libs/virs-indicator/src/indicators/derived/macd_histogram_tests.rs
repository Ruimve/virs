use super::macd_histogram::compute;
use crate::indicators::test_utils::kline;

#[test]
fn histogram_positive_in_accelerating_uptrend() {
    // 加速上涨：MACD 远离 Signal，histogram > 0
    let klines: Vec<_> = (0..60).map(|i| {
        let close = 100.0 + (i as f64).powi(2) * 0.1;
        kline(close - 2.0, close + 1.0, close - 3.0, close, 1000.0)
    }).collect();
    let val = compute(&klines, 12, 26, 9).unwrap();
    assert!(val > 0.0, "MACD histogram should be positive in accelerating uptrend, got {val}");
}

#[test]
fn histogram_negative_in_accelerating_downtrend() {
    // 加速下跌：MACD 远离 Signal，histogram < 0
    let klines: Vec<_> = (0..60).map(|i| {
        let close = 200.0 - (i as f64).powi(2) * 0.1;
        kline(close + 2.0, close + 3.0, close - 1.0, close, 1000.0)
    }).collect();
    let val = compute(&klines, 12, 26, 9).unwrap();
    assert!(val < 0.0, "MACD histogram should be negative in accelerating downtrend, got {val}");
}

#[test]
fn histogram_errors_on_insufficient_data() {
    let klines: Vec<_> = (0..20).map(|i| kline(100.0, 101.0, 99.0, 100.0 + i as f64, 1000.0)).collect();
    assert!(compute(&klines, 12, 26, 9).is_err());
}
