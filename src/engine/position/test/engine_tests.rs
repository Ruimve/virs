use super::common::*;
use super::super::engine::{test_helpers, EngineInner, handle_open_position, handle_close_position, handle_modify_position, handle_place_order, handle_cancel_order, handle_cancel_all_orders, handle_sync_positions, handle_ws_order_update};
use super::super::exchange::Exchange;
use super::super::types::*;
use super::super::error::PositionEngineError;
use std::sync::Arc;
use tokio::sync::broadcast;

fn make_inner() -> Arc<EngineInner> {
    let config = make_engine_config();
    let exchange = Box::new(MockExchange::new("binance"));
    test_helpers::make_test_inner(config, exchange)
}

fn make_inner_with_mock() -> (Arc<EngineInner>, MockExchange) {
    let config = make_engine_config();
    let mock = MockExchange::new("binance");
    let inner = test_helpers::make_test_inner(config, Box::new(mock.clone()));
    (inner, mock)
}

fn subscribe(inner: &Arc<EngineInner>) -> broadcast::Receiver<EngineEvent> {
    inner.event_tx.subscribe()
}

fn collect_events(rx: &mut broadcast::Receiver<EngineEvent>, max: usize) -> Vec<EngineEvent> {
    test_helpers::collect_events(rx, max)
}

fn insert_position(inner: &Arc<EngineInner>, key: (String, String, PositionSide), pos: Position) {
    inner.position_id_index.insert(pos.id, key.clone());
    inner.positions.insert(key, pos);
}

fn insert_order(inner: &Arc<EngineInner>, order: Order) {
    if let Some(ref eoid) = order.exchange_order_id {
        inner.exchange_order_id_index.insert(eoid.clone(), order.id);
    }
    inner.orders.insert(order.id, order);
}

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

#[test]
fn test_engine_config_deserialization_partial() {
    let json = r#"{"engine_id": "partial"}"#;
    let config: crate::engine::position::config::EngineConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.engine_id, "partial");
    assert_eq!(config.sync_interval_secs, 10);
    assert_eq!(config.risk.max_leverage, 20);
}

#[test]
fn test_risk_config_deserialization_partial() {
    let json = r#"{"max_leverage": 5}"#;
    let config: crate::engine::position::config::RiskConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.max_leverage, 5);
    assert_eq!(config.max_drawdown_pct, 0.15);
    assert_eq!(config.max_consecutive_losses, 5);
}

#[tokio::test]
async fn test_mock_exchange_name() {
    let mock = MockExchange::new("binance");
    assert_eq!(mock.name(), "binance");
}

#[tokio::test]
async fn test_mock_exchange_ticker() {
    let mock = MockExchange::new("binance");
    mock.set_ticker("BTC/USDT", 50000.0).await;
    let ticker = mock.get_ticker("BTC/USDT").await.unwrap();
    assert_eq!(ticker.symbol, "BTC/USDT");
    assert!((ticker.price - 50000.0).abs() < 0.01);
}

#[tokio::test]
async fn test_mock_exchange_place_order_tracking() {
    let mock = MockExchange::new("binance");
    let order = make_filled_order("BTC/USDT", Side::Buy, 1.0, 50000.0);
    mock.set_place_order_ok(order).await;
    let params = PlaceOrderParams {
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 1.0,
        price: None,
        reduce_only: false,
        position_side: Some(PositionSide::Long),
        position_id: None,
    };
    mock.place_order(params).await.unwrap();
    assert_eq!(mock.place_order_count().await, 1);
}

#[tokio::test]
async fn test_mock_exchange_cancel_order_tracking() {
    let mock = MockExchange::new("binance");
    let order = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(50000.0));
    mock.set_cancel_order_ok(order).await;
    mock.cancel_order("BTC/USDT", "123").await.unwrap();
    assert_eq!(mock.cancel_order_count().await, 1);
}

#[tokio::test]
async fn test_mock_exchange_market_type() {
    let mock = MockExchange::new("binance");
    assert_eq!(mock.market_type(), MarketType::Perpetual);
}

#[tokio::test]
async fn test_mock_exchange_balance() {
    let mock = MockExchange::new("binance");
    mock.set_balance(50000.0).await;
    let balance = mock.get_balance().await.unwrap();
    assert!((balance.total - 50000.0).abs() < 0.01);
}

#[tokio::test]
async fn test_mock_exchange_funding_rate() {
    let mock = MockExchange::new("binance");
    let fr = mock.get_funding_rate("BTC/USDT").await.unwrap();
    assert_eq!(fr.symbol, "BTC/USDT");
    assert!((fr.rate - 0.0001).abs() < 1e-8);
}

#[tokio::test]
async fn test_mock_exchange_fee_rates() {
    let mock = MockExchange::new("binance");
    let fees = mock.get_fee_rates("BTC/USDT").await.unwrap();
    assert_eq!(fees.symbol, "BTC/USDT");
    assert!((fees.taker_rate - 0.0005).abs() < 1e-8);
}

#[tokio::test]
async fn test_mock_exchange_get_ticker_not_found() {
    let mock = MockExchange::new("binance");
    let result = mock.get_ticker("ETH/USDT").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mock_exchange_place_order_default() {
    let mock = MockExchange::new("binance");
    let params = PlaceOrderParams {
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 0.5,
        price: None,
        reduce_only: false,
        position_side: Some(PositionSide::Long),
        position_id: None,
    };
    let order = mock.place_order(params).await.unwrap();
    assert_eq!(order.symbol, "BTC/USDT");
    assert_eq!(order.side, Side::Buy);
    assert!((order.amount - 0.5).abs() < 1e-8);
    assert_eq!(order.status, OrderStatus::Filled);
}

#[tokio::test]
async fn test_mock_exchange_cancel_order_not_configured() {
    let mock = MockExchange::new("binance");
    let result = mock.cancel_order("BTC/USDT", "123").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mock_exchange_cancel_all_default() {
    let mock = MockExchange::new("binance");
    let result = mock.cancel_all_orders("BTC/USDT").await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_mock_exchange_set_leverage() {
    let mock = MockExchange::new("binance");
    mock.set_leverage("BTC/USDT", 10).await.unwrap();
    let log = mock.leverage_log().await;
    assert_eq!(log.len(), 1);
    assert_eq!(log[0], ("BTC/USDT".to_string(), 10));
}

#[tokio::test]
async fn test_mock_exchange_get_order_not_implemented() {
    let mock = MockExchange::new("binance");
    let result = mock.get_order("BTC/USDT", "123").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_handle_open_position_success() {
    let (inner, mock) = make_inner_with_mock();
    let filled = make_filled_order("BTC/USDT", Side::Buy, 1.0, 50000.0);
    mock.set_place_order_ok(filled).await;
    let mut rx = subscribe(&inner);

    handle_open_position(
        &inner,
        "binance".to_string(),
        "BTC/USDT".to_string(),
        PositionSide::Long,
        1.0,
        Some(5),
        OrderType::Market,
        None,
        None,
        None,
        None,
    )
    .await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long));
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.side, PositionSide::Long);
    assert!((pos.size - 1.0).abs() < 1e-8);
    assert!((pos.entry_price - 50000.0).abs() < 0.01);
    assert_eq!(pos.status, PositionStatus::Open);

    let events = collect_events(&mut rx, 10);
    let has_opened = events.iter().any(|e| matches!(e, EngineEvent::PositionOpened { .. }));
    let has_placed = events.iter().any(|e| matches!(e, EngineEvent::OrderPlaced { .. }));
    assert!(has_opened);
    assert!(has_placed);
}

#[tokio::test]
async fn test_handle_open_position_already_exists() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    let pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long), pos);

    handle_open_position(
        &inner,
        "binance".to_string(),
        "BTC/USDT".to_string(),
        PositionSide::Long,
        1.0,
        Some(5),
        OrderType::Market,
        None,
        None,
        None,
        None,
    )
    .await;

    let events = collect_events(&mut rx, 10);
    let has_failed = events.iter().any(|e| matches!(e, EngineEvent::OrderFailed { .. }));
    assert!(has_failed);
}

#[tokio::test]
async fn test_handle_open_position_exchange_error() {
    let (inner, mock) = make_inner_with_mock();
    mock.set_place_order_err(PositionEngineError::Exchange("connection error".to_string())).await;
    let mut rx = subscribe(&inner);

    handle_open_position(
        &inner,
        "binance".to_string(),
        "BTC/USDT".to_string(),
        PositionSide::Long,
        1.0,
        Some(5),
        OrderType::Market,
        None,
        None,
        None,
        None,
    )
    .await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long));
    assert!(pos.is_none());

    let events = collect_events(&mut rx, 10);
    let has_failed = events.iter().any(|e| matches!(e, EngineEvent::OrderFailed { .. }));
    assert!(has_failed);
}

#[tokio::test]
async fn test_handle_open_position_short_side() {
    let (inner, mock) = make_inner_with_mock();
    let filled = make_filled_order("BTC/USDT", Side::Sell, 1.0, 50000.0);
    mock.set_place_order_ok(filled).await;

    handle_open_position(
        &inner,
        "binance".to_string(),
        "BTC/USDT".to_string(),
        PositionSide::Short,
        1.0,
        Some(5),
        OrderType::Market,
        None,
        None,
        None,
        None,
    )
    .await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Short));
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().side, PositionSide::Short);
}

#[tokio::test]
async fn test_handle_open_position_with_stop_loss_take_profit() {
    let (inner, mock) = make_inner_with_mock();
    let filled = make_filled_order("BTC/USDT", Side::Buy, 1.0, 50000.0);
    mock.set_place_order_ok(filled).await;

    handle_open_position(
        &inner,
        "binance".to_string(),
        "BTC/USDT".to_string(),
        PositionSide::Long,
        1.0,
        Some(5),
        OrderType::Market,
        None,
        Some(45000.0),
        Some(60000.0),
        None,
    )
    .await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long));
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert!((pos.stop_loss.unwrap() - 45000.0).abs() < 0.01);
    assert!((pos.take_profit.unwrap() - 60000.0).abs() < 0.01);
}

#[tokio::test]
async fn test_handle_open_position_leverage_set() {
    let (inner, mock) = make_inner_with_mock();
    let filled = make_filled_order("BTC/USDT", Side::Buy, 1.0, 50000.0);
    mock.set_place_order_ok(filled).await;

    handle_open_position(
        &inner,
        "binance".to_string(),
        "BTC/USDT".to_string(),
        PositionSide::Long,
        1.0,
        Some(10),
        OrderType::Market,
        None,
        None,
        None,
        None,
    )
    .await;

    let log = mock.leverage_log().await;
    assert_eq!(log.len(), 1);
    assert_eq!(log[0], ("BTC/USDT".to_string(), 10));
}

#[tokio::test]
async fn test_handle_open_position_limit_order() {
    let (inner, mock) = make_inner_with_mock();
    let mut order = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(49000.0));
    order.status = OrderStatus::Open;
    order.filled = 0.0;
    order.remaining = 1.0;
    mock.set_place_order_ok(order).await;

    handle_open_position(
        &inner,
        "binance".to_string(),
        "BTC/USDT".to_string(),
        PositionSide::Long,
        1.0,
        Some(5),
        OrderType::Limit,
        Some(49000.0),
        None,
        None,
        None,
    )
    .await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long));
    assert!(pos.is_some());
}

#[tokio::test]
async fn test_handle_open_position_with_strategy_id() {
    let (inner, mock) = make_inner_with_mock();
    let filled = make_filled_order("BTC/USDT", Side::Buy, 1.0, 50000.0);
    mock.set_place_order_ok(filled).await;

    handle_open_position(
        &inner,
        "binance".to_string(),
        "BTC/USDT".to_string(),
        PositionSide::Long,
        1.0,
        Some(5),
        OrderType::Market,
        None,
        None,
        None,
        Some("grid_strategy".to_string()),
    )
    .await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long));
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().strategy_id, Some("grid_strategy".to_string()));
}

#[tokio::test]
async fn test_handle_close_position_success() {
    let (inner, mock) = make_inner_with_mock();
    let filled = make_filled_order("BTC/USDT", Side::Sell, 1.0, 55000.0);
    mock.set_place_order_ok(filled).await;
    let mut rx = subscribe(&inner);

    let pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    let pos_id = pos.id;
    let key = ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long);
    insert_position(&inner, key.clone(), pos);

    handle_close_position(&inner, pos_id, OrderType::Market, None).await;

    let pos = inner.positions.get(&key).unwrap();
    assert_eq!(pos.status, PositionStatus::Closing);

    let events = collect_events(&mut rx, 10);
    let has_placed = events.iter().any(|e| matches!(e, EngineEvent::OrderPlaced { .. }));
    assert!(has_placed);
}

#[tokio::test]
async fn test_handle_close_position_not_found() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    handle_close_position(&inner, uuid::Uuid::new_v4(), OrderType::Market, None).await;

    let events = collect_events(&mut rx, 10);
    let has_failed = events.iter().any(|e| matches!(e, EngineEvent::OrderFailed { .. }));
    assert!(has_failed);
}

#[tokio::test]
async fn test_handle_close_position_zero_size() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    let mut pos = make_position_side("BTC/USDT", PositionSide::Long, 0.0, 50000.0, 5);
    let pos_id = pos.id;
    let key = ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long);
    insert_position(&inner, key, pos);

    handle_close_position(&inner, pos_id, OrderType::Market, None).await;

    let events = collect_events(&mut rx, 10);
    let has_failed = events.iter().any(|e| matches!(e, EngineEvent::OrderFailed { .. }));
    assert!(has_failed);
}

#[tokio::test]
async fn test_handle_close_position_exchange_error() {
    let (inner, mock) = make_inner_with_mock();
    mock.set_place_order_err(PositionEngineError::Exchange("error".to_string())).await;
    let mut rx = subscribe(&inner);

    let pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    let pos_id = pos.id;
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long), pos);

    handle_close_position(&inner, pos_id, OrderType::Market, None).await;

    let events = collect_events(&mut rx, 10);
    let has_failed = events.iter().any(|e| matches!(e, EngineEvent::OrderFailed { .. }));
    assert!(has_failed);
}

#[tokio::test]
async fn test_handle_close_position_short_side() {
    let (inner, mock) = make_inner_with_mock();
    let filled = make_filled_order("BTC/USDT", Side::Buy, 1.0, 45000.0);
    mock.set_place_order_ok(filled).await;

    let pos = make_position_side("BTC/USDT", PositionSide::Short, 1.0, 50000.0, 5);
    let pos_id = pos.id;
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Short), pos);

    handle_close_position(&inner, pos_id, OrderType::Market, None).await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Short));
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().status, PositionStatus::Closing);
}

#[tokio::test]
async fn test_handle_modify_position_success() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    let pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    let pos_id = pos.id;
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long), pos);

    handle_modify_position(&inner, pos_id, Some(45000.0), Some(60000.0)).await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long));
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert!((pos.stop_loss.unwrap() - 45000.0).abs() < 0.01);
    assert!((pos.take_profit.unwrap() - 60000.0).abs() < 0.01);

    let events = collect_events(&mut rx, 10);
    let has_modified = events.iter().any(|e| matches!(e, EngineEvent::PositionModified { .. }));
    assert!(has_modified);
}

#[tokio::test]
async fn test_handle_modify_position_not_found() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    handle_modify_position(&inner, uuid::Uuid::new_v4(), Some(45000.0), None).await;

    let events = collect_events(&mut rx, 10);
    let has_failed = events.iter().any(|e| matches!(e, EngineEvent::OrderFailed { .. }));
    assert!(has_failed);
}

#[tokio::test]
async fn test_handle_modify_position_clear_sl_tp() {
    let (inner, _) = make_inner_with_mock();

    let mut pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    pos.stop_loss = Some(45000.0);
    pos.take_profit = Some(60000.0);
    let pos_id = pos.id;
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long), pos);

    handle_modify_position(&inner, pos_id, None, None).await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long));
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert!(pos.stop_loss.is_none());
    assert!(pos.take_profit.is_none());
}

#[tokio::test]
async fn test_handle_place_order_success() {
    let (inner, mock) = make_inner_with_mock();
    let filled = make_filled_order("BTC/USDT", Side::Buy, 1.0, 50000.0);
    mock.set_place_order_ok(filled).await;
    let mut rx = subscribe(&inner);

    let params = PlaceOrderParams {
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 1.0,
        price: None,
        reduce_only: false,
        position_side: Some(PositionSide::Long),
        position_id: None,
    };

    handle_place_order(&inner, params).await;

    let events = collect_events(&mut rx, 10);
    let has_placed = events.iter().any(|e| matches!(e, EngineEvent::OrderPlaced { .. }));
    assert!(has_placed);
}

#[tokio::test]
async fn test_handle_place_order_exchange_error() {
    let (inner, mock) = make_inner_with_mock();
    mock.set_place_order_err(PositionEngineError::Exchange("error".to_string())).await;
    let mut rx = subscribe(&inner);

    let params = PlaceOrderParams {
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 1.0,
        price: None,
        reduce_only: false,
        position_side: Some(PositionSide::Long),
        position_id: None,
    };

    handle_place_order(&inner, params).await;

    let events = collect_events(&mut rx, 10);
    let has_failed = events.iter().any(|e| matches!(e, EngineEvent::OrderFailed { .. }));
    assert!(has_failed);
}

#[tokio::test]
async fn test_handle_cancel_order_success() {
    let (inner, mock) = make_inner_with_mock();
    let mut order = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(49000.0));
    order.exchange_order_id = Some("ex_123".to_string());
    let order_id = order.id;
    insert_order(&inner, order.clone());

    let cancelled = {
        let mut o = order.clone();
        o.status = OrderStatus::Canceled;
        o
    };
    mock.set_cancel_order_ok(cancelled).await;
    let mut rx = subscribe(&inner);

    handle_cancel_order(&inner, order_id).await;

    let events = collect_events(&mut rx, 10);
    let has_canceled = events.iter().any(|e| matches!(e, EngineEvent::OrderCanceled { .. }));
    assert!(has_canceled);
}

#[tokio::test]
async fn test_handle_cancel_order_not_found() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    handle_cancel_order(&inner, uuid::Uuid::new_v4()).await;

    let events = collect_events(&mut rx, 10);
    let has_failed = events.iter().any(|e| matches!(e, EngineEvent::OrderFailed { .. }));
    assert!(has_failed);
}

#[tokio::test]
async fn test_handle_cancel_order_no_exchange_id() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    let mut order = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(49000.0));
    order.exchange_order_id = None;
    let order_id = order.id;
    insert_order(&inner, order);

    handle_cancel_order(&inner, order_id).await;

    let events = collect_events(&mut rx, 10);
    let has_failed = events.iter().any(|e| matches!(e, EngineEvent::OrderFailed { .. }));
    assert!(has_failed);
}

#[tokio::test]
async fn test_handle_cancel_order_exchange_error() {
    let (inner, mock) = make_inner_with_mock();
    let mut order = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(49000.0));
    order.exchange_order_id = Some("ex_123".to_string());
    let order_id = order.id;
    insert_order(&inner, order);

    mock.set_cancel_order_ok({
        let mut o = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(49000.0));
        o.status = OrderStatus::Canceled;
        o
    }).await;

    handle_cancel_order(&inner, order_id).await;

    assert_eq!(mock.cancel_order_count().await, 1);
}

#[tokio::test]
async fn test_handle_cancel_all_orders_with_symbol() {
    let (inner, mock) = make_inner_with_mock();
    let order1 = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(49000.0));
    mock.set_cancel_all_ok(vec![order1]).await;
    let mut rx = subscribe(&inner);

    handle_cancel_all_orders(&inner, None, Some("BTC/USDT".to_string())).await;

    let events = collect_events(&mut rx, 10);
    let has_canceled = events.iter().any(|e| matches!(e, EngineEvent::OrderCanceled { .. }));
    assert!(has_canceled);
}

#[tokio::test]
async fn test_handle_cancel_all_orders_with_position_id() {
    let (inner, mock) = make_inner_with_mock();
    let pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    let pos_id = pos.id;
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long), pos);

    let order1 = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(49000.0));
    mock.set_cancel_all_ok(vec![order1]).await;

    handle_cancel_all_orders(&inner, Some(pos_id), None).await;
}

#[tokio::test]
async fn test_handle_cancel_all_orders_no_args() {
    let (inner, mock) = make_inner_with_mock();
    mock.set_cancel_all_ok(vec![]).await;

    handle_cancel_all_orders(&inner, None, None).await;
}

#[tokio::test]
async fn test_handle_sync_positions_success() {
    let (inner, mock) = make_inner_with_mock();
    mock.set_positions(vec![ExchangePosition {
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        size: 1.0,
        entry_price: 50000.0,
        leverage: 5,
        unrealized_pnl: 100.0,
        liquidation_price: Some(45000.0),
    }]).await;
    let mut rx = subscribe(&inner);

    handle_sync_positions(&inner).await;

    let events = collect_events(&mut rx, 10);
    let has_synced = events.iter().any(|e| matches!(e, EngineEvent::PositionSynced { .. }));
    assert!(has_synced);
}

#[tokio::test]
async fn test_handle_sync_positions_exchange_error() {
    let (inner, mock) = make_inner_with_mock();
    mock.set_get_positions_fail(true).await;
    let mut rx = subscribe(&inner);

    handle_sync_positions(&inner).await;

    let events = collect_events(&mut rx, 10);
    let has_synced = events.iter().any(|e| matches!(e, EngineEvent::PositionSynced { .. }));
    assert!(!has_synced);
}

#[tokio::test]
async fn test_handle_ws_order_update_filled() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    let pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    let pos_id = pos.id;
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long), pos);

    let mut order = make_order("BTC/USDT", Side::Buy, OrderType::Market, 1.0, None);
    order.position_id = pos_id;
    order.exchange_order_id = Some("ex_ws_001".to_string());
    let order_id = order.id;
    insert_order(&inner, order);

    handle_ws_order_update(
        &inner,
        "ex_ws_001",
        "BTC/USDT",
        OrderStatus::Filled,
        1.0,
        0.0,
        50000.0,
        1.0,
        0.5,
        chrono::Utc::now(),
    )
    .await;

    let events = collect_events(&mut rx, 10);
    let has_filled = events.iter().any(|e| matches!(e, EngineEvent::OrderFilled { .. }));
    assert!(has_filled);
}

#[tokio::test]
async fn test_handle_ws_order_update_partial_fill() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    let pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    let pos_id = pos.id;
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long), pos);

    let mut order = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 2.0, Some(49000.0));
    order.position_id = pos_id;
    order.exchange_order_id = Some("ex_ws_002".to_string());
    order.filled = 0.5;
    let order_id = order.id;
    insert_order(&inner, order);

    handle_ws_order_update(
        &inner,
        "ex_ws_002",
        "BTC/USDT",
        OrderStatus::PartiallyFilled,
        1.0,
        1.0,
        49000.0,
        2.0,
        0.3,
        chrono::Utc::now(),
    )
    .await;

    let events = collect_events(&mut rx, 10);
    let has_partial = events.iter().any(|e| matches!(e, EngineEvent::OrderPartiallyFilled { .. }));
    assert!(has_partial);
}

#[tokio::test]
async fn test_handle_ws_order_update_unknown_order() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    handle_ws_order_update(
        &inner,
        "ex_unknown",
        "BTC/USDT",
        OrderStatus::Filled,
        1.0,
        0.0,
        50000.0,
        1.0,
        0.0,
        chrono::Utc::now(),
    )
    .await;

    let events = collect_events(&mut rx, 10);
    assert!(events.is_empty());
}

#[tokio::test]
async fn test_handle_ws_order_update_reduce_only_closes_position() {
    let (inner, _mock) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    let pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    let pos_id = pos.id;
    let key = ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long);
    insert_position(&inner, key.clone(), pos);

    let mut order = make_order("BTC/USDT", Side::Sell, OrderType::Market, 1.0, None);
    order.position_id = pos_id;
    order.reduce_only = true;
    order.exchange_order_id = Some("ex_close_001".to_string());
    let order_id = order.id;
    insert_order(&inner, order);

    handle_ws_order_update(
        &inner,
        "ex_close_001",
        "BTC/USDT",
        OrderStatus::Filled,
        1.0,
        0.0,
        55000.0,
        1.0,
        0.5,
        chrono::Utc::now(),
    )
    .await;

    let pos_exists = inner.positions.contains_key(&key);
    assert!(!pos_exists);

    let events = collect_events(&mut rx, 10);
    let has_closed = events.iter().any(|e| matches!(e, EngineEvent::PositionClosed { .. }));
    assert!(has_closed);
}

#[tokio::test]
async fn test_handle_ws_order_update_reduce_only_partial_close() {
    let (inner, _) = make_inner_with_mock();
    let mut rx = subscribe(&inner);

    let pos = make_position_side("BTC/USDT", PositionSide::Long, 2.0, 50000.0, 5);
    let pos_id = pos.id;
    let key = ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long);
    insert_position(&inner, key.clone(), pos);

    let mut order = make_order("BTC/USDT", Side::Sell, OrderType::Market, 1.0, None);
    order.position_id = pos_id;
    order.reduce_only = true;
    order.exchange_order_id = Some("ex_partial_close".to_string());
    let order_id = order.id;
    insert_order(&inner, order);

    handle_ws_order_update(
        &inner,
        "ex_partial_close",
        "BTC/USDT",
        OrderStatus::Filled,
        1.0,
        0.0,
        55000.0,
        1.0,
        0.5,
        chrono::Utc::now(),
    )
    .await;

    let pos = inner.positions.get(&key);
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert!((pos.size - 1.0).abs() < 1e-8);
    assert_eq!(pos.status, PositionStatus::Open);
}

#[tokio::test]
async fn test_handle_ws_order_update_open_order_updates_entry_price() {
    let (inner, _) = make_inner_with_mock();

    let pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    let pos_id = pos.id;
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long), pos);

    let mut order = make_order("BTC/USDT", Side::Buy, OrderType::Market, 1.0, None);
    order.position_id = pos_id;
    order.reduce_only = false;
    order.exchange_order_id = Some("ex_add_001".to_string());
    let order_id = order.id;
    insert_order(&inner, order);

    handle_ws_order_update(
        &inner,
        "ex_add_001",
        "BTC/USDT",
        OrderStatus::Filled,
        1.0,
        0.0,
        52000.0,
        1.0,
        0.5,
        chrono::Utc::now(),
    )
    .await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long));
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert!((pos.size - 2.0).abs() < 1e-8);
    assert!((pos.entry_price - 51000.0).abs() < 0.01);
}

#[tokio::test]
async fn test_handle_ws_order_update_short_position_pnl() {
    let (inner, _) = make_inner_with_mock();

    let pos = make_position_side("BTC/USDT", PositionSide::Short, 1.0, 50000.0, 5);
    let pos_id = pos.id;
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Short), pos);

    let mut order = make_order("BTC/USDT", Side::Buy, OrderType::Market, 1.0, None);
    order.position_id = pos_id;
    order.reduce_only = true;
    order.exchange_order_id = Some("ex_short_close".to_string());
    let order_id = order.id;
    insert_order(&inner, order);

    handle_ws_order_update(
        &inner,
        "ex_short_close",
        "BTC/USDT",
        OrderStatus::Filled,
        1.0,
        0.0,
        48000.0,
        1.0,
        0.5,
        chrono::Utc::now(),
    )
    .await;

    let pos = inner.positions.get(&("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Short));
    assert!(pos.is_none());
}

#[tokio::test]
async fn test_inner_positions_dashmap() {
    let inner = make_inner();
    assert!(inner.positions.is_empty());

    let pos = make_position_side("BTC/USDT", PositionSide::Long, 1.0, 50000.0, 5);
    insert_position(&inner, ("binance".to_string(), "BTC/USDT".to_string(), PositionSide::Long), pos);
    assert_eq!(inner.positions.len(), 1);

    let pos2 = make_position_side("ETH/USDT", PositionSide::Short, 10.0, 3000.0, 3);
    insert_position(&inner, ("binance".to_string(), "ETH/USDT".to_string(), PositionSide::Short), pos2);
    assert_eq!(inner.positions.len(), 2);
}

#[tokio::test]
async fn test_inner_orders_dashmap() {
    let inner = make_inner();
    assert!(inner.orders.is_empty());

    let order = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(49000.0));
    insert_order(&inner, order);
    assert_eq!(inner.orders.len(), 1);
}

#[tokio::test]
async fn test_open_position_command_construction() {
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

#[tokio::test]
async fn test_close_position_command_construction() {
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

#[tokio::test]
async fn test_modify_position_command_construction() {
    let pos_id = uuid::Uuid::new_v4();
    let cmd = EngineCommand::ModifyPosition {
        position_id: pos_id,
        stop_loss: Some(45000.0),
        take_profit: None,
    };

    match cmd {
        EngineCommand::ModifyPosition { position_id, stop_loss, take_profit } => {
            assert_eq!(position_id, pos_id);
            assert_eq!(stop_loss, Some(45000.0));
            assert!(take_profit.is_none());
        }
        _ => panic!("Expected ModifyPosition command"),
    }
}

#[tokio::test]
async fn test_cancel_order_command_construction() {
    let order_id = uuid::Uuid::new_v4();
    let cmd = EngineCommand::CancelOrder { order_id };

    match cmd {
        EngineCommand::CancelOrder { order_id: oid } => {
            assert_eq!(oid, order_id);
        }
        _ => panic!("Expected CancelOrder command"),
    }
}

#[tokio::test]
async fn test_cancel_all_orders_command_construction() {
    let pos_id = uuid::Uuid::new_v4();
    let cmd = EngineCommand::CancelAllOrders {
        position_id: Some(pos_id),
        symbol: Some("BTC/USDT".to_string()),
    };

    match cmd {
        EngineCommand::CancelAllOrders { position_id, symbol } => {
            assert_eq!(position_id, Some(pos_id));
            assert_eq!(symbol, Some("BTC/USDT".to_string()));
        }
        _ => panic!("Expected CancelAllOrders command"),
    }
}

#[tokio::test]
async fn test_sync_positions_command_construction() {
    let cmd = EngineCommand::SyncPositions;
    assert!(matches!(cmd, EngineCommand::SyncPositions));
}

#[tokio::test]
async fn test_shutdown_command_construction() {
    let cmd = EngineCommand::Shutdown;
    assert!(matches!(cmd, EngineCommand::Shutdown));
}

#[tokio::test]
async fn test_place_order_command_construction() {
    let params = PlaceOrderParams {
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 1.0,
        price: None,
        reduce_only: false,
        position_side: Some(PositionSide::Long),
        position_id: None,
    };
    let cmd = EngineCommand::PlaceOrder { params };

    match cmd {
        EngineCommand::PlaceOrder { params } => {
            assert_eq!(params.symbol, "BTC/USDT");
            assert_eq!(params.side, Side::Buy);
        }
        _ => panic!("Expected PlaceOrder command"),
    }
}

#[tokio::test]
async fn test_mock_exchange_positions() {
    let mock = MockExchange::new("binance");
    mock.set_positions(vec![ExchangePosition {
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        size: 1.0,
        entry_price: 50000.0,
        leverage: 5,
        unrealized_pnl: 100.0,
        liquidation_price: Some(45000.0),
    }]).await;

    let positions = mock.get_positions(None).await.unwrap();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].symbol, "BTC/USDT");
    assert!((positions[0].size - 1.0).abs() < 1e-8);
}

#[tokio::test]
async fn test_mock_exchange_open_orders() {
    let mock = MockExchange::new("binance");
    let order = make_order("BTC/USDT", Side::Buy, OrderType::Limit, 1.0, Some(49000.0));
    mock.set_open_orders(vec![order]).await;

    let orders = mock.get_open_orders(None).await.unwrap();
    assert_eq!(orders.len(), 1);
}

#[tokio::test]
async fn test_mock_exchange_place_order_err() {
    let mock = MockExchange::new("binance");
    mock.set_place_order_err(PositionEngineError::Exchange("insufficient margin".to_string())).await;

    let params = PlaceOrderParams {
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 1.0,
        price: None,
        reduce_only: false,
        position_side: Some(PositionSide::Long),
        position_id: None,
    };

    let result = mock.place_order(params).await;
    assert!(result.is_err());
    assert_eq!(mock.place_order_count().await, 1);
}
