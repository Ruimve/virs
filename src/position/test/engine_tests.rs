//! 引擎集成测试
//!
//! 注意：PositionEngine 需要 PgPool，完整的集成测试需要数据库环境。
//! 当前只测试引擎的公开 API 构造和基本类型交互。

use super::common::*;
use super::super::engine::PositionEngine;
use super::super::types::*;

// ============================================================
// 引擎构造 (2 tests)
// ============================================================

#[test]
fn test_engine_config_default() {
    let config = make_engine_config();
    assert_eq!(config.engine_id, "test_engine");
    assert_eq!(config.sync_interval_secs, 1);
    assert_eq!(config.poll_interval_secs, 1);
}

#[test]
fn test_engine_config_custom_risk() {
    let mut config = make_engine_config();
    config.risk.max_leverage = 5;
    config.risk.max_drawdown_pct = 0.10;
    assert_eq!(config.risk.max_leverage, 5);
    assert!((config.risk.max_drawdown_pct - 0.10).abs() < 0.001);
}

// ============================================================
// MockExchange 验证 (4 tests)
// ============================================================

#[tokio::test]
async fn test_mock_exchange_name() {
    let mock = MockExchange::new("binance");
    assert_eq!(mock.name(), "binance");
}

#[tokio::test]
async fn test_mock_exchange_ticker() {
    let mock = MockExchange::new("binance");
    mock.set_ticker("BTC/USDT", 50000.0);

    let ticker = mock.get_ticker("BTC/USDT").await.unwrap();
    assert_eq!(ticker.symbol, "BTC/USDT");
    assert!((ticker.price - 50000.0).abs() < 0.01);
}

#[tokio::test]
async fn test_mock_exchange_place_order_tracking() {
    let mock = MockExchange::new("binance");
    let order = make_filled_order("BTC/USDT", Side::Buy, 1.0, 50000.0);
    mock.set_place_order_ok(order);

    let params = PlaceOrderParams {
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 1.0,
        price: None,
        reduce_only: false,
        position_side: Some(PositionSide::Long),
    };

    mock.place_order(params).await.unwrap();
    assert_eq!(mock.place_order_count(), 1);
}

#[tokio::test]
async fn test_mock_exchange_cancel_order_tracking() {
    let mock = MockExchange::new("binance");
    let order = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(50000.0));
    mock.set_cancel_order_ok(order);

    mock.cancel_order("BTC/USDT", "123").await.unwrap();
    assert_eq!(mock.cancel_order_count(), 1);
}

// ============================================================
// 命令/事件构造验证 (2 tests)
// ============================================================

#[test]
fn test_open_position_command() {
    let cmd = EngineCommand::OpenPosition {
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        size: 1.0,
        leverage: Some(10),
        order_type: OrderType::Market,
        price: None,
        stop_loss: Some(45000.0),
        take_profit: Some(60000.0),
        strategy_id: None,
    };

    match cmd {
        EngineCommand::OpenPosition { symbol, side, size, leverage, .. } => {
            assert_eq!(symbol, "BTC/USDT");
            assert_eq!(side, PositionSide::Long);
            assert_eq!(size, 1.0);
            assert_eq!(leverage, Some(10));
        }
        _ => panic!("Expected OpenPosition command"),
    }
}

#[test]
fn test_close_position_command() {
    let pos_id = uuid::Uuid::new_v4();
    let cmd = EngineCommand::ClosePosition {
        position_id: pos_id,
        order_type: OrderType::Market,
        price: None,
    };

    match cmd {
        EngineCommand::ClosePosition { position_id, order_type, .. } => {
            assert_eq!(position_id, pos_id);
            assert_eq!(order_type, OrderType::Market);
        }
        _ => panic!("Expected ClosePosition command"),
    }
}

// ============================================================
// Config serde 测试 (2 tests)
// ============================================================

#[test]
fn test_engine_config_deserialization_partial() {
    // 只传 engine_id，其余用默认值
    let json = r#"{"engine_id": "partial"}"#;
    let config: crate::position::config::EngineConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.engine_id, "partial");
    assert_eq!(config.sync_interval_secs, 10); // 默认值
    assert_eq!(config.risk.max_leverage, 20);   // 默认值
}

#[test]
fn test_risk_config_deserialization_partial() {
    // 只传 max_leverage，其余用默认值
    let json = r#"{"max_leverage": 5}"#;
    let config: crate::position::config::RiskConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.max_leverage, 5);
    assert_eq!(config.max_drawdown_pct, 0.15); // 默认值
    assert_eq!(config.max_consecutive_losses, 5); // 默认值
}
