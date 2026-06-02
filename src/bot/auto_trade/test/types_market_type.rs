/**
 * 测试 types::MarketType 枚举
 * - as_str: Perpetual → "perpetual", Spot → "spot"
 * - from_str_lossy: "spot" → Spot, 其他 → Perpetual
 * - Display trait: 与 as_str 一致
 * - serde 序列化/反序列化
 */
use crate::bot::auto_trade::types::MarketType;

#[test]
fn perpetual_as_str() {
    assert_eq!(MarketType::Perpetual.as_str(), "perpetual");
}

#[test]
fn spot_as_str() {
    assert_eq!(MarketType::Spot.as_str(), "spot");
}

#[test]
fn from_str_lossy_spot() {
    assert_eq!(MarketType::from_str_lossy("spot"), MarketType::Spot);
}

#[test]
fn from_str_lossy_perpetual() {
    assert_eq!(MarketType::from_str_lossy("perpetual"), MarketType::Perpetual);
}

#[test]
fn from_str_lossy_unknown_defaults_to_perpetual() {
    assert_eq!(MarketType::from_str_lossy("margin"), MarketType::Perpetual);
    assert_eq!(MarketType::from_str_lossy(""), MarketType::Perpetual);
}

#[test]
fn from_str_lossy_case_insensitive() {
    assert_eq!(MarketType::from_str_lossy("SPOT"), MarketType::Spot);
    assert_eq!(MarketType::from_str_lossy("Spot"), MarketType::Spot);
}

#[test]
fn display_trait() {
    assert_eq!(format!("{}", MarketType::Perpetual), "perpetual");
    assert_eq!(format!("{}", MarketType::Spot), "spot");
}

#[test]
fn serde_roundtrip_perpetual() {
    let json = serde_json::to_string(&MarketType::Perpetual).unwrap();
    assert_eq!(json, "\"perpetual\"");
    let parsed: MarketType = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, MarketType::Perpetual);
}

#[test]
fn serde_roundtrip_spot() {
    let json = serde_json::to_string(&MarketType::Spot).unwrap();
    assert_eq!(json, "\"spot\"");
    let parsed: MarketType = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, MarketType::Spot);
}

#[test]
fn equality() {
    assert_eq!(MarketType::Perpetual, MarketType::Perpetual);
    assert_ne!(MarketType::Perpetual, MarketType::Spot);
}
