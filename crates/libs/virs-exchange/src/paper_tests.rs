//! Unit tests for paper.rs compute_paper_liquidation_price.

use virs_types::enums::PositionSide;

use crate::paper::compute_paper_liquidation_price;

// ============================================================
// TC-P1: compute_paper_liquidation_price
// ============================================================

#[test]
fn p1_1_long_normal() {
    // entry=50000, lev=10 → 50000 * (1 - 1/10) = 50000 * 0.9 = 45000
    let result = compute_paper_liquidation_price(50000.0, PositionSide::Long, 10);
    assert_eq!(result, Some(45000.0));
}

#[test]
fn p1_2_short_normal() {
    // entry=50000, lev=10 → 50000 * (1 + 1/10) = 55000
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
    // entry=50000, lev=1 → 50000 * (1 - 1/1) = 50000 * 0 = 0
    let result = compute_paper_liquidation_price(50000.0, PositionSide::Long, 1);
    assert_eq!(result, Some(0.0));
}

#[test]
fn p1_8_leverage_one_short() {
    // entry=50000, lev=1 → 50000 * (1 + 1/1) = 50000 * 2 = 100000
    let result = compute_paper_liquidation_price(50000.0, PositionSide::Short, 1);
    assert!(result.is_some());
    assert!((result.unwrap() - 100000.0).abs() < 0.01);
}
