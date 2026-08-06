use super::sma::sma_at_from;

#[test]
fn sma_of_constant_series() {
    let series = vec![100.0; 20];
    let val = sma_at_from(&series, 19, 10).unwrap();
    assert!((val - 100.0).abs() < 0.001, "SMA of constant should equal the constant");
}

#[test]
fn sma_of_increasing_series() {
    let series: Vec<f64> = (0..20).map(|i| i as f64).collect();
    let val = sma_at_from(&series, 19, 5).unwrap();
    let expected: f64 = (15.0 + 16.0 + 17.0 + 18.0 + 19.0) / 5.0;
    assert!((val - expected).abs() < 0.001, "SMA should be {expected}, got {val}");
}

#[test]
fn sma_handles_nan_values() {
    let mut series = vec![100.0; 20];
    series[5] = f64::NAN;
    let val = sma_at_from(&series, 19, 5).unwrap();
    assert!(val.is_finite(), "SMA should produce finite value with NaN filtered out");
}

#[test]
fn sma_errors_on_empty_series() {
    let series: Vec<f64> = vec![];
    assert!(sma_at_from(&series, 0, 10).is_err());
}

#[test]
fn sma_errors_on_zero_period() {
    let series = vec![100.0; 10];
    assert!(sma_at_from(&series, 5, 0).is_err());
}
