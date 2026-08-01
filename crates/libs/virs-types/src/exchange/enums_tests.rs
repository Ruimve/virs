use super::*;
use crate::exchange::{MarketType, MarginMode, PositionMode};


#[test]
fn e3_1_market_type_display() {
    assert_eq!(MarketType::Perpetual.to_string(), "perpetual");
}

#[test]
fn e3_2_margin_mode_serde() {
    let json = serde_json::to_string(&MarginMode::Cross).unwrap();
    assert_eq!(json, "\"cross\"");
    let de: MarginMode = serde_json::from_str(&json).unwrap();
    assert_eq!(de, MarginMode::Cross);
}

#[test]
fn e3_3_position_mode_serde() {
    let json = serde_json::to_string(&PositionMode::Hedge).unwrap();
    let de: PositionMode = serde_json::from_str(&json).unwrap();
    assert_eq!(de, PositionMode::Hedge);
}
