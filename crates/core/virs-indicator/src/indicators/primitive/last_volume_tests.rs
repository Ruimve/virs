use super::last_volume::compute;
use crate::indicators::test_utils::{uptrend_klines, kline};

#[test]
fn returns_second_to_last_volume() {

    let klines = uptrend_klines(30, 100.0, 1.0);
    let val = compute(&klines).unwrap();
    let expected = klines[klines.len() - 2].volume;
    assert_eq!(val, expected, "Should return last completed kline volume");
}

#[test]
fn returns_correct_volume_for_manual_klines() {
    let klines = vec![
        kline(100.0, 105.0, 95.0, 103.0, 500.0),
        kline(103.0, 107.0, 100.0, 106.0, 800.0),
        kline(106.0, 110.0, 104.0, 109.0, 1200.0),
    ];
    let val = compute(&klines).unwrap();
    assert_eq!(val, 800.0, "Should return volume of kline at index 1 (len-2)");
}

#[test]
fn errors_on_empty_klines() {
    let klines: Vec<virs_type::Kline> = vec![];
    assert!(compute(&klines).is_err());
}

#[test]
fn errors_on_single_kline() {
    let klines = vec![kline(100.0, 105.0, 95.0, 103.0, 500.0)];
    assert!(compute(&klines).is_err());
}
