//! Unit tests for bot.rs methods.

use crate::bot::BotPositionSide;

// ============================================================
// TC-B1: BotPositionSide::as_str
// ============================================================

#[test]
fn b1_1_long_as_str() {
    assert_eq!(BotPositionSide::Long.as_str(), "long");
}

#[test]
fn b1_2_short_as_str() {
    assert_eq!(BotPositionSide::Short.as_str(), "short");
}

// ============================================================
// TC-B2: BotPositionSide::is_long
// ============================================================

#[test]
fn b2_1_long_is_long() {
    assert!(BotPositionSide::Long.is_long());
    assert!(!BotPositionSide::Short.is_long());
}

// ============================================================
// TC-B3: BotPositionSide::is_short
// ============================================================

#[test]
fn b3_1_short_is_short() {
    assert!(BotPositionSide::Short.is_short());
    assert!(!BotPositionSide::Long.is_short());
}
