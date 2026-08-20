use std::sync::Arc;

use crate::adapters::order_executor::convert_pe_event;
use chrono::Utc;
use uuid::Uuid;
use virs_type::OrderEvent;
use virs_type::{OrderType, PositionSide, Side, TradeType};
use virs_type::{EngineEvent, Trade};
use virs_type::{CcxtOrder, CcxtOrderStatus, ExecutionType};

fn make_order(side: Side) -> CcxtOrder {
    CcxtOrder {
        order_id: 123,
        client_order_id: "CL456".to_string(),
        symbol: "BTCUSDT".to_string(),
        side,
        order_type: OrderType::Limit,
        position_side: PositionSide::Long,
        original_order_type: OrderType::Limit,
        status: CcxtOrderStatus::Filled,
        execution_type: ExecutionType::Trade,
        orig_qty: "1.0".to_string(),
        original_price: "100.0".to_string(),
        avg_fill_price: "101.0".to_string(),
        filled_qty: "1.0".to_string(),
        last_fill_qty: "1.0".to_string(),
        last_fill_price: "101.0".to_string(),
        stop_price: "0".to_string(),
        commission: "0.1".to_string(),
        commission_asset: "USDT".to_string(),
        realized_pnl: "0".to_string(),
        reduce_only: false,
        is_maker: false,
        close_position: None,
        time_in_force: "GTC".to_string(),
        working_type: "CONTRACT_PRICE".to_string(),
        bids_notional: "0".to_string(),
        ask_notional: "0".to_string(),
        activation_price: None,
        callback_rate: None,
        price_protection: false,
        stp_mode: "NONE".to_string(),
        price_match_mode: "NONE".to_string(),
        gtd_auto_cancel_time: 0,
        expiry_reason: "0".to_string(),
        si: Some(0),
        ss: Some(0),
        trade_time: 0,
        trade_id: 0,
        modify_id: None,
        envelope_event_type: "ORDER_TRADE_UPDATE".to_string(),
        envelope_event_time: 0,
        envelope_transaction_time: 0,
    }
}

fn make_trade() -> Trade {
    Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTCUSDT".to_string(),
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
    let order = Arc::new(make_order(Side::Buy));
    let event = EngineEvent::OrderPlaced { order };
    let result = convert_pe_event(event);
    assert!(result.is_some());
    match result.unwrap() {
        OrderEvent::OrderPlaced { order } => {
            assert_eq!(order.side, Side::Buy);
            assert_eq!(order.symbol, "BTCUSDT");
            assert!((order.fill_price.unwrap() - 101.0).abs() < 1e-10);
        }
        _ => panic!("Expected OrderPlaced"),
    }
}

#[test]
fn o1_2_convert_order_filled() {
    let order = Arc::new(make_order(Side::Sell));
    let event = EngineEvent::OrderFilled {
        order,
        trade: make_trade(),
    };
    let result = convert_pe_event(event);
    assert!(result.is_some());
    match result.unwrap() {
        OrderEvent::OrderFilled { order } => {
            assert_eq!(order.side, Side::Sell);
        }
        _ => panic!("Expected OrderFilled"),
    }
}

#[test]
fn o1_3_convert_order_canceled() {
    let order = Arc::new(make_order(Side::Buy));
    let order_id = Uuid::from_u128(order.order_id as u128);
    let event = EngineEvent::OrderCanceled { order };
    let result = convert_pe_event(event);
    assert!(result.is_some());
    match result.unwrap() {
        OrderEvent::OrderCanceled {
            order_id: id,
            client_order_id,
            symbol,
        } => {
            assert_eq!(id, order_id);
            assert!(client_order_id.is_some());
            assert_eq!(symbol.as_deref(), Some("BTCUSDT"));
        }
        _ => panic!("Expected OrderCanceled"),
    }
}

#[test]
fn o1_4_convert_order_failed() {
    let event = EngineEvent::OrderFailed {
        client_order_id: "CL456".to_string(),
        reason: "Insufficient balance".to_string(),
    };
    let result = convert_pe_event(event);
    assert!(result.is_some());
    match result.unwrap() {
        OrderEvent::OrderFailed {
            order_id: _,
            client_order_id,
            reason,
        } => {
            assert_eq!(reason, "Insufficient balance");
            assert_eq!(client_order_id.as_deref(), Some("CL456"));
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
    let pos = virs_type::Position {
        id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTCUSDT".to_string(),
        side: virs_type::PositionSide::Long,
        status: virs_type::PositionStatus::Open,
        quantity: 1.0,
        entry_price: 100.0,
        realized_pnl: 0.0,
        client_order_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let event = EngineEvent::PositionOpened { position: pos };
    let result = convert_pe_event(event);
    assert!(result.is_none());
}
