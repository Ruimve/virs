use super::common::*;
use crate::bot::semi_automatic_grid::ports::*;
use uuid::Uuid;

#[test]
fn grid_side_as_str() {
    assert_eq!(GridSide::Buy.as_str(), "buy");
    assert_eq!(GridSide::Sell.as_str(), "sell");
}

#[test]
fn grid_side_serialization() {
    let buy_json = serde_json::to_string(&GridSide::Buy).unwrap();
    assert_eq!(buy_json, r#""Buy""#);
    let sell_json = serde_json::to_string(&GridSide::Sell).unwrap();
    assert_eq!(sell_json, r#""Sell""#);
    let buy: GridSide = serde_json::from_str(r#""Buy""#).unwrap();
    assert_eq!(buy, GridSide::Buy);
    let sell: GridSide = serde_json::from_str(r#""Sell""#).unwrap();
    assert_eq!(sell, GridSide::Sell);
}

#[test]
fn grid_order_command_place_order() {
    let cmd = GridOrderCommand::PlaceOrder {
        symbol: "BTCUSDT".to_string(),
        side: GridSide::Buy,
        amount: 0.001,
        price: Some(50000.0),
        reduce_only: false,
    };
    match &cmd {
        GridOrderCommand::PlaceOrder { symbol, side, amount, price, reduce_only } => {
            assert_eq!(symbol, "BTCUSDT");
            assert_eq!(*side, GridSide::Buy);
            assert!((amount - 0.001).abs() < f64::EPSILON);
            assert_eq!(*price, Some(50000.0));
            assert!(!reduce_only);
        }
        _ => panic!("Expected PlaceOrder variant"),
    }
}

#[test]
fn grid_order_command_cancel_all() {
    let cmd = GridOrderCommand::CancelAllOrders { symbol: Some("ETHUSDT".to_string()) };
    match &cmd {
        GridOrderCommand::CancelAllOrders { symbol } => assert_eq!(symbol, &Some("ETHUSDT".to_string())),
        _ => panic!("Expected CancelAllOrders variant"),
    }
    let cmd_none = GridOrderCommand::CancelAllOrders { symbol: None };
    match &cmd_none {
        GridOrderCommand::CancelAllOrders { symbol } => assert!(symbol.is_none()),
        _ => panic!("Expected CancelAllOrders variant"),
    }
}

#[test]
fn grid_bot_config_construction() {
    let id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let config = GridBotConfig {
        id, user_id,
        name: "Test Bot".to_string(),
        symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(),
        grid_count: 10,
        upper_price: 60000.0,
        lower_price: 50000.0,
        grid_profit_pct: 0.5,
        quantity_per_grid: 100.0,
        dynamic_adjust: true,
        adjust_interval_secs: 300,
        market_regime: Some("trending".to_string()),
        system_prompt: Some("test prompt".to_string()),
    };
    assert_eq!(config.id, id);
    assert_eq!(config.user_id, user_id);
    assert_eq!(config.name, "Test Bot");
    assert_eq!(config.symbol, "BTCUSDT");
    assert_eq!(config.exchange, "binance");
    assert_eq!(config.grid_count, 10);
    assert!((config.upper_price - 60000.0).abs() < f64::EPSILON);
    assert!((config.lower_price - 50000.0).abs() < f64::EPSILON);
    assert!((config.grid_profit_pct - 0.5).abs() < f64::EPSILON);
    assert!((config.quantity_per_grid - 100.0).abs() < f64::EPSILON);
    assert!(config.dynamic_adjust);
    assert_eq!(config.adjust_interval_secs, 300);
    assert_eq!(config.market_regime, Some("trending".to_string()));
    assert_eq!(config.system_prompt, Some("test prompt".to_string()));
}
