use super::bandwidth_bars::compute;
use crate::indicators::test_utils::uptrend_klines;

#[test]
fn returns_valid_width_5_bars_ago() {
    let klines = uptrend_klines(60, 100.0, 1.0);
    let val = compute(&klines, 20, 2, 5).unwrap();
    assert!(val > 0.0, "Bandwidth 5 bars ago should be positive, got {val}");
}

#[test]
fn errors_when_bars_ago_exceeds_data() {
    let klines = uptrend_klines(30, 100.0, 1.0);
    assert!(compute(&klines, 20, 2, 50).is_err());
}

#[test]
fn errors_on_insufficient_data() {
    let klines = uptrend_klines(10, 100.0, 1.0);
    assert!(compute(&klines, 20, 2, 5).is_err());
}
