use crate::bot::semi_automatic_grid::adapters::convert_pe_event;
use crate::bot::semi_automatic_grid::ports::*;
use crate::engine::position::types::*;
use chrono::Utc;
use uuid::Uuid;

fn make_order(side: Side, fill_price: Option<f64>) -> Order {
    Order {
        id: Uuid::new_v4(), position_id: Uuid::new_v4(),
        exchange_order_id: None, client_order_id: None,
        exchange: "binance".to_string(), symbol: "BTCUSDT".to_string(),
        side, order_type: OrderType::Limit,
        request_price: Some(50000.0), fill_price,
        amount: 0.001, filled: 0.001, remaining: 0.0,
        status: OrderStatus::Filled, reduce_only: false,
        fee: 0.0, fee_currency: "USDT".to_string(),
        slippage: None, created_at: Utc::now(), updated_at: Utc::now(),
    }
}

fn make_trade() -> Trade {
    Trade {
        id: Uuid::new_v4(), position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(), exchange: "binance".to_string(),
        symbol: "BTCUSDT".to_string(), side: Side::Buy,
        price: 50000.0, amount: 0.001, fee: 0.05,
        fee_currency: "USDT".to_string(), pnl: 0.0,
        trade_type: TradeType::Open, created_at: Utc::now(),
    }
}

#[test]
fn convert_order_placed_buy() {
    let order = make_order(Side::Buy, None);
    let event = EngineEvent::OrderPlaced { order };
    let result = convert_pe_event(event).unwrap();
    match result {
        GridOrderEvent::OrderPlaced { order: info } => {
            assert_eq!(info.side, GridSide::Buy);
            assert_eq!(info.request_price, Some(50000.0));
            assert!((info.filled - 0.001).abs() < f64::EPSILON);
        }
        _ => panic!("Expected OrderPlaced"),
    }
}

#[test]
fn convert_order_placed_sell() {
    let order = make_order(Side::Sell, None);
    let event = EngineEvent::OrderPlaced { order };
    let result = convert_pe_event(event).unwrap();
    match result {
        GridOrderEvent::OrderPlaced { order: info } => {
            assert_eq!(info.side, GridSide::Sell);
        }
        _ => panic!("Expected OrderPlaced"),
    }
}

#[test]
fn convert_order_filled() {
    let order = make_order(Side::Buy, Some(51000.0));
    let trade = make_trade();
    let event = EngineEvent::OrderFilled { order, trade };
    let result = convert_pe_event(event).unwrap();
    match result {
        GridOrderEvent::OrderFilled { order: info } => {
            assert_eq!(info.side, GridSide::Buy);
            assert_eq!(info.fill_price, Some(51000.0));
            assert_eq!(info.request_price, Some(50000.0));
            assert!((info.filled - 0.001).abs() < f64::EPSILON);
        }
        _ => panic!("Expected OrderFilled"),
    }
}

#[test]
fn convert_order_canceled() {
    let order = make_order(Side::Buy, None);
    let order_id = order.id;
    let event = EngineEvent::OrderCanceled { order };
    let result = convert_pe_event(event).unwrap();
    match result {
        GridOrderEvent::OrderCanceled { order_id: id } => assert_eq!(id, order_id),
        _ => panic!("Expected OrderCanceled"),
    }
}

#[test]
fn convert_order_failed() {
    let order_id = Uuid::new_v4();
    let reason = "Insufficient margin".to_string();
    let event = EngineEvent::OrderFailed { order_id, reason: reason.clone() };
    let result = convert_pe_event(event).unwrap();
    match result {
        GridOrderEvent::OrderFailed { order_id: id, reason: r } => {
            assert_eq!(id, order_id);
            assert_eq!(r, reason);
        }
        _ => panic!("Expected OrderFailed"),
    }
}

#[test]
fn convert_risk_alert() {
    let level = "CloseAll".to_string();
    let message = "High exposure detected".to_string();
    let event = EngineEvent::RiskAlert { level: level.clone(), message: message.clone() };
    let result = convert_pe_event(event).unwrap();
    match result {
        GridOrderEvent::RiskAlert { level: l, message: m } => {
            assert_eq!(l, level);
            assert_eq!(m, message);
        }
        _ => panic!("Expected RiskAlert"),
    }
}

#[test]
fn convert_liquidation_warning() {
    let event = EngineEvent::LiquidationWarning {
        position_id: Uuid::new_v4(),
        symbol: "BTCUSDT".to_string(),
        liquidation_price: 45000.0,
        current_price: 46000.0,
    };
    let result = convert_pe_event(event).unwrap();
    match result {
        GridOrderEvent::LiquidationWarning { symbol, liquidation_price, current_price } => {
            assert_eq!(symbol, "BTCUSDT");
            assert!((liquidation_price - 45000.0).abs() < f64::EPSILON);
            assert!((current_price - 46000.0).abs() < f64::EPSILON);
        }
        _ => panic!("Expected LiquidationWarning"),
    }
}

#[test]
fn convert_unknown_event() {
    let position = Position {
        id: Uuid::new_v4(), engine_id: "test".to_string(), strategy_id: None,
        exchange: "binance".to_string(), symbol: "BTCUSDT".to_string(),
        side: PositionSide::Long, status: PositionStatus::Open,
        size: 0.001, entry_price: 50000.0, current_price: 51000.0,
        leverage: 5, margin: 10.0, unrealized_pnl: 1.0, realized_pnl: 0.0,
        stop_loss: None, take_profit: None, liquidation_price: None,
        opened_at: Utc::now(), updated_at: Utc::now(), closed_at: None,
        metadata: serde_json::Value::Null,
    };
    assert!(convert_pe_event(EngineEvent::PositionOpened { position }).is_none());
    assert!(convert_pe_event(EngineEvent::PositionClosed { position: Position {
        id: Uuid::new_v4(), engine_id: "test".to_string(), strategy_id: None,
        exchange: "binance".to_string(), symbol: "BTCUSDT".to_string(),
        side: PositionSide::Long, status: PositionStatus::Closed,
        size: 0.0, entry_price: 50000.0, current_price: 51000.0,
        leverage: 5, margin: 0.0, unrealized_pnl: 0.0, realized_pnl: 1.0,
        stop_loss: None, take_profit: None, liquidation_price: None,
        opened_at: Utc::now(), updated_at: Utc::now(), closed_at: Some(Utc::now()),
        metadata: serde_json::Value::Null,
    }}).is_none());
    assert!(convert_pe_event(EngineEvent::PositionModified {
        position_id: Uuid::new_v4(), stop_loss: Some(49000.0), take_profit: Some(55000.0),
    }).is_none());
    let order = make_order(Side::Buy, Some(50000.0));
    let trade = make_trade();
    assert!(convert_pe_event(EngineEvent::OrderPartiallyFilled { order, trade }).is_none());
    assert!(convert_pe_event(EngineEvent::PositionSynced { positions: vec![] }).is_none());
}
