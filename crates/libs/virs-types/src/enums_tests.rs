//! Unit tests for enums.rs methods.

use crate::enums::*;

// ============================================================
// TC-E1: Side::as_str
// ============================================================

#[test]
fn e1_1_buy_as_str() {
    assert_eq!(Side::Buy.as_str(), "buy");
}

#[test]
fn e1_2_sell_as_str() {
    assert_eq!(Side::Sell.as_str(), "sell");
}

// ============================================================
// TC-E2: Side::is_opening_for
// ============================================================

#[test]
fn e2_1_buy_long_is_opening() {
    assert!(Side::Buy.is_opening_for(PositionSide::Long));
}

#[test]
fn e2_2_sell_short_is_opening() {
    assert!(Side::Sell.is_opening_for(PositionSide::Short));
}

#[test]
fn e2_3_sell_long_not_opening() {
    assert!(!Side::Sell.is_opening_for(PositionSide::Long));
}

#[test]
fn e2_4_buy_short_not_opening() {
    assert!(!Side::Buy.is_opening_for(PositionSide::Short));
}

// ============================================================
// TC-E3: Side::is_closing_for
// ============================================================

#[test]
fn e3_1_sell_long_is_closing() {
    assert!(Side::Sell.is_closing_for(PositionSide::Long));
}

#[test]
fn e3_2_buy_short_is_closing() {
    assert!(Side::Buy.is_closing_for(PositionSide::Short));
}

#[test]
fn e3_3_buy_long_not_closing() {
    assert!(!Side::Buy.is_closing_for(PositionSide::Long));
}

#[test]
fn e3_4_sell_short_not_closing() {
    assert!(!Side::Sell.is_closing_for(PositionSide::Short));
}

// ============================================================
// TC-E4: PositionSide::as_str
// ============================================================

#[test]
fn e4_1_long_as_str() {
    assert_eq!(PositionSide::Long.as_str(), "long");
}

#[test]
fn e4_2_short_as_str() {
    assert_eq!(PositionSide::Short.as_str(), "short");
}

#[test]
fn e4_3_both_as_str() {
    assert_eq!(PositionSide::Both.as_str(), "both");
}

// ============================================================
// TC-E5/E6: PositionSide::is_long/is_short
// ============================================================

#[test]
fn e5_1_long_is_long() {
    assert!(PositionSide::Long.is_long());
    assert!(!PositionSide::Long.is_short());
}

#[test]
fn e5_2_short_is_short() {
    assert!(!PositionSide::Short.is_long());
    assert!(PositionSide::Short.is_short());
}

// ============================================================
// TC-E7-E10: OrderStatus methods
// ============================================================

#[test]
fn e7_1_filled_is_filled() {
    assert!(OrderStatus::Filled.is_filled());
    assert!(!OrderStatus::Open.is_filled());
}

#[test]
fn e8_1_open_is_open() {
    assert!(OrderStatus::Open.is_open());
}

#[test]
fn e8_2_partially_filled_is_open() {
    assert!(OrderStatus::PartiallyFilled.is_open());
}

#[test]
fn e8_3_filled_not_open() {
    assert!(!OrderStatus::Filled.is_open());
}

#[test]
fn e9_1_canceled_is_canceled() {
    assert!(OrderStatus::Canceled.is_canceled());
    assert!(!OrderStatus::Open.is_canceled());
}

#[test]
fn e10_1_terminal_states() {
    assert!(OrderStatus::Filled.is_terminal());
    assert!(OrderStatus::Canceled.is_terminal());
    assert!(OrderStatus::Failed.is_terminal());
}

#[test]
fn e10_2_non_terminal_states() {
    assert!(!OrderStatus::Open.is_terminal());
    assert!(!OrderStatus::PartiallyFilled.is_terminal());
    assert!(!OrderStatus::Pending.is_terminal());
}

// ============================================================
// TC-E11-E13: PositionStatus methods
// ============================================================

#[test]
fn e11_1_open_is_open() {
    assert!(PositionStatus::Open.is_open());
    assert!(!PositionStatus::Closed.is_open());
}

#[test]
fn e12_1_closed_is_closed() {
    assert!(PositionStatus::Closed.is_closed());
    assert!(!PositionStatus::Open.is_closed());
}

#[test]
fn e13_1_empty_is_empty() {
    assert!(PositionStatus::Empty.is_empty());
    assert!(!PositionStatus::Open.is_empty());
}

// ============================================================
// TC-E14/E15: EngineState methods
// ============================================================

#[test]
fn e14_1_running_is_running() {
    assert!(EngineState::Running.is_running());
    assert!(!EngineState::Stopped.is_running());
}

#[test]
fn e15_1_stopped_is_stopped() {
    assert!(EngineState::Stopped.is_stopped());
    assert!(!EngineState::Running.is_stopped());
}

// ============================================================
// TC-E16/E17: StrategyStatus methods
// ============================================================

#[test]
fn e16_1_running_is_running() {
    assert!(StrategyStatus::Running.is_running());
    assert!(!StrategyStatus::Stopped.is_running());
}

#[test]
fn e17_1_stopped_is_stopped() {
    assert!(StrategyStatus::Stopped.is_stopped());
    assert!(!StrategyStatus::Running.is_stopped());
}
