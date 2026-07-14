use virs_types::enums::PositionSide;

use crate::paper::compute_paper_liquidation_price;


#[test]
fn p1_1_long_normal() {

    let result = compute_paper_liquidation_price(50000.0, PositionSide::Long, 10);
    assert_eq!(result, Some(45000.0));
}

#[test]
fn p1_2_short_normal() {

    let result = compute_paper_liquidation_price(50000.0, PositionSide::Short, 10);
    assert!(result.is_some());
    assert!((result.unwrap() - 55000.0).abs() < 0.01);
}

#[test]
fn p1_3_leverage_zero() {
    let result = compute_paper_liquidation_price(50000.0, PositionSide::Long, 0);
    assert_eq!(result, None);
}

#[test]
fn p1_4_entry_price_zero() {
    let result = compute_paper_liquidation_price(0.0, PositionSide::Long, 10);
    assert_eq!(result, None);
}

#[test]
fn p1_5_entry_price_negative() {
    let result = compute_paper_liquidation_price(-100.0, PositionSide::Long, 10);
    assert_eq!(result, None);
}

#[test]
fn p1_7_leverage_one_long() {

    let result = compute_paper_liquidation_price(50000.0, PositionSide::Long, 1);
    assert_eq!(result, Some(0.0));
}

#[test]
fn p1_8_leverage_one_short() {

    let result = compute_paper_liquidation_price(50000.0, PositionSide::Short, 1);
    assert!(result.is_some());
    assert!((result.unwrap() - 100000.0).abs() < 0.01);
}
