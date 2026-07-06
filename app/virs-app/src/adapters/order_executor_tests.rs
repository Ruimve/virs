//! Unit tests for adapters/order_executor.rs

use crate::adapters::order_executor::convert_pe_event;
use chrono::Utc;
use uuid::Uuid;
use virs_types::bot::{OrderEvent, OrderSide};
use virs_types::enums::{OrderStatus, OrderType, Side, TradeType};
use virs_types::position::{EngineEvent, PositionOrder, Trade};

fn make_order(side: Side) -> PositionOrder {
    PositionOrder {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        exchange_order_id: Some("EX123".to_string()),
        client_order_id: Some("CL456".to_string()),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side,
        order_type: OrderType::Limit,
        request_price: Some(100.0),
        fill_price: Some(101.0),
        amount: 1.0,
        filled: 1.0,
        remaining: 0.0,
        status: OrderStatus::Filled,
        reduce_only: false,
        fee: 0.1,
        fee_currency: "USDT".to_string(),
        slippage: Some(0.5),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn make_trade() -> Trade {
    Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        price: 101.0,
        amount: 1.0,
        fee: 0.1,
        fee_currency: "USDT".to_string(),
        pnl: 0.0,
        trade_type: TradeType::Open,
        created_at: Utc::now(),
    }
}

#[test]
fn o1_1_convert_order_placed() {
    let order = make_order(Side::Buy);
    let event = EngineEvent::OrderPlaced { order };
    let result = convert_pe_event(event);
    assert!(result.is_some());
    match result.unwrap() {
        OrderEvent::OrderPlaced { order } => {
            assert_eq!(order.side, OrderSide::Buy);
            assert_eq!(order.symbol, "BTC/USDT");
            assert!((order.fill_price.unwrap() - 101.0).abs() < 1e-10);
        }
        _ => panic!("Expected OrderPlaced"),
    }
}

#[test]
fn o1_2_convert_order_filled() {
    let order = make_order(Side::Sell);
    let event = EngineEvent::OrderFilled {
        order,
        trade: make_trade(),
    };
    let result = convert_pe_event(event);
    assert!(result.is_some());
    match result.unwrap() {
        OrderEvent::OrderFilled { order } => {
            assert_eq!(order.side, OrderSide::Sell);
        }
        _ => panic!("Expected OrderFilled"),
    }
}

#[test]
fn o1_3_convert_order_canceled() {
    let order = make_order(Side::Buy);
    let order_id = order.id;
    let event = EngineEvent::OrderCanceled { order };
    let result = convert_pe_event(event);
    assert!(result.is_some());
    match result.unwrap() {
        OrderEvent::OrderCanceled {
            order_id: id,
            symbol,
        } => {
            assert_eq!(id, order_id);
            assert_eq!(symbol.as_deref(), Some("BTC/USDT"));
        }
        _ => panic!("Expected OrderCanceled"),
    }
}

#[test]
fn o1_4_convert_order_failed() {
    let oid = Uuid::new_v4();
    let event = EngineEvent::OrderFailed {
        order_id: oid,
        reason: "Insufficient balance".to_string(),
    };
    let result = convert_pe_event(event);
    assert!(result.is_some());
    match result.unwrap() {
        OrderEvent::OrderFailed { order_id, reason } => {
            assert_eq!(order_id, oid);
            assert_eq!(reason, "Insufficient balance");
        }
        _ => panic!("Expected OrderFailed"),
    }
}

#[test]
fn o1_5_convert_risk_alert() {
    let event = EngineEvent::RiskAlert {
        level: "critical".to_string(),
        message: "Max drawdown exceeded".to_string(),
    };
    let result = convert_pe_event(event);
    assert!(result.is_some());
    match result.unwrap() {
        OrderEvent::RiskAlert { level, message } => {
            assert_eq!(level, "critical");
            assert_eq!(message, "Max drawdown exceeded");
        }
        _ => panic!("Expected RiskAlert"),
    }
}

#[test]
fn o1_6_convert_position_opened_none() {
    // PositionOpened is not mapped to OrderEvent → None
    let pos = virs_types::position::Position {
        id: Uuid::new_v4(),
        strategy_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: virs_types::enums::PositionSide::Long,
        status: virs_types::enums::PositionStatus::Open,
        size: 1.0,
        entry_price: 100.0,
        current_price: 100.0,
        leverage: 10,
        margin: 10.0,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss: None,
        take_profit: None,
        liquidation_price: None,
        opened_at: Utc::now(),
        updated_at: Utc::now(),
        closed_at: None,
        metadata: serde_json::Value::Null,
    };
    let event = EngineEvent::PositionOpened { position: pos };
    let result = convert_pe_event(event);
    assert!(result.is_none());
}
