use super::ema_gap_trend::compute;
use crate::indicators::test_utils::uptrend_klines;

#[test]
fn returns_valid_string() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let val = compute(&klines, 20, 50).unwrap();
    assert!(
        val == "扩大" || val == "缩小" || val == "持平",
        "Trend must be one of 扩大/缩小/持平, got {val}"
    );
}

#[test]
fn expanding_in_accelerating_uptrend() {

    let klines: Vec<_> = (0..60).map(|i| {
        let close = 100.0 + (i as f64).powi(2) * 0.05;
        crate::indicators::test_utils::kline(close - 1.0, close + 1.0, close - 2.0, close, 1000.0)
    }).collect();
    let val = compute(&klines, 20, 50).unwrap();
    assert_eq!(val, "扩大", "Gap should be expanding in accelerating trend, got {val}");
}

#[test]
fn errors_on_insufficient_data() {
    let klines = uptrend_klines(30, 100.0, 1.0);
    assert!(compute(&klines, 20, 50).is_err());
}
