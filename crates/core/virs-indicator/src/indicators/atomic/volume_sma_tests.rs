use super::volume_sma::volume_sma_at;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn volume_sma_of_constant_volume() {
    let klines: Vec<_> = (0..30).map(|_| kline(100.0, 105.0, 95.0, 100.0, 1000.0)).collect();
    let val = volume_sma_at(&klines, 28, 5).unwrap();
    assert!((val - 1000.0).abs() < 0.001, "Volume SMA should be 1000.0, got {val}");
}

#[test]
fn volume_sma_of_increasing_volume() {
    let klines = uptrend_klines(30, 100.0, 1.0);
    let val = volume_sma_at(&klines, 28, 5).unwrap();

    let vols: Vec<f64> = (24..=28).map(|i| 1000.0 + i as f64 * 10.0).collect();
    let expected: f64 = vols.iter().sum::<f64>() / 5.0;
    assert!((val - expected).abs() < 0.001, "Volume SMA should be {expected}, got {val}");
}

#[test]
fn volume_sma_errors_on_insufficient_data() {
    let klines = uptrend_klines(5, 100.0, 1.0);
    assert!(volume_sma_at(&klines, 3, 10).is_err());
}
