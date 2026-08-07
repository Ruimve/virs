use crate::ws::{kline_event_to_json, orderbook_event_to_json, position_to_ws_json};
use chrono::Utc;
use uuid::Uuid;
use virs_type::Candle;
use virs_type::OrderBookLevel;
use virs_type::OrderBookEvent;
use virs_type::{KlineEvent, KlineEventType, Timeframe};
use virs_type::{PositionSide, PositionStatus};
use virs_type::Position;


fn ws_value<T: serde::Serialize>(v: T) -> serde_json::Value {
    serde_json::to_value(v).unwrap()
}


#[test]
fn w1_1_position_all_fields() {
    let pos = make_position(PositionSide::Long, PositionStatus::Open);
    let json = ws_value(position_to_ws_json(&pos, Some(45000.0), Some(55000.0)));
    assert_eq!(json["type"], "position_updated");
    assert_eq!(json["symbol"], "BTC/USDT");
    assert_eq!(json["exchange"], "binance");
    assert_eq!(json["side"], "long");
    assert_eq!(json["status"], "open");
    assert_eq!(json["quantity"], 1.0);
    assert_eq!(json["entry_price"], 50000.0);
    assert_eq!(json["stop_loss"], 45000.0);
    assert_eq!(json["take_profit"], 55000.0);
}

#[test]
fn w1_2_position_optional_fields_none() {
    let pos = make_position(PositionSide::Short, PositionStatus::Closed);
    let json = ws_value(position_to_ws_json(&pos, None, None));
    assert!(json["stop_loss"].is_null());
    assert!(json["take_profit"].is_null());
}

#[test]
fn w1_3_position_type_field() {
    let pos = make_position(PositionSide::Long, PositionStatus::Open);
    let json = ws_value(position_to_ws_json(&pos, None, None));
    assert_eq!(json["type"], "position_updated");
}


#[test]
fn w2_1_kline_normal() {
    let event = make_kline_event(KlineEventType::Update);
    let json = ws_value(kline_event_to_json(&event));
    assert_eq!(json["exchange"], "binance");
    assert_eq!(json["symbol"], "BTC/USDT");
    assert_eq!(json["timeframe"], "1m");
    assert_eq!(json["candle"]["open"], 50000.0);
    assert_eq!(json["candle"]["close"], 50100.0);
    assert_eq!(json["candle"]["closed"], false);
    assert_eq!(json["event_type"], "Update");
}

#[test]
fn w2_2_kline_event_types() {
    assert_eq!(ws_value(kline_event_to_json(&make_kline_event(KlineEventType::Update)))["event_type"], "Update");
    assert_eq!(ws_value(kline_event_to_json(&make_kline_event(KlineEventType::Closed)))["event_type"], "Closed");
    assert_eq!(ws_value(kline_event_to_json(&make_kline_event(KlineEventType::Backfilled)))["event_type"], "Backfilled");
}

#[test]
fn w2_3_kline_timeframe_format() {
    let event = make_kline_event(KlineEventType::Update);
    let json = ws_value(kline_event_to_json(&event));
    assert_eq!(json["timeframe"], "1m");
}


#[test]
fn w3_1_orderbook_normal() {
    let event = make_orderbook_event();
    let json = ws_value(orderbook_event_to_json(&event));
    assert_eq!(json["exchange"], "binance");
    assert_eq!(json["symbol"], "BTC/USDT");
    assert_eq!(json["bids"].as_array().unwrap().len(), 2);
    assert_eq!(json["asks"].as_array().unwrap().len(), 2);
    assert_eq!(json["bids"][0][0], 50000.0);
    assert_eq!(json["bids"][0][1], 1.5);
}

#[test]
fn w3_2_orderbook_empty_levels() {
    let event = OrderBookEvent {
        exchange: "binance".into(),
        symbol: "BTC/USDT".into(),
        bids: vec![],
        asks: vec![],
        timestamp: 1700000000000,
    };
    let json = ws_value(orderbook_event_to_json(&event));
    assert!(json["bids"].as_array().unwrap().is_empty());
    assert!(json["asks"].as_array().unwrap().is_empty());
}

#[test]
fn w3_3_orderbook_level_format() {
    let event = make_orderbook_event();
    let json = ws_value(orderbook_event_to_json(&event));
    let first_bid = &json["bids"][0];
    assert!(first_bid.is_array());
    assert_eq!(first_bid[0], 50000.0);
    assert_eq!(first_bid[1], 1.5);
}


fn make_position(side: PositionSide, status: PositionStatus) -> Position {
    Position {
        id: Uuid::nil(),
        exchange: "binance".into(),
        symbol: "BTC/USDT".into(),
        side,
        status,
        quantity: 1.0,
        entry_price: 50000.0,
        realized_pnl: 0.0,
        client_order_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn make_kline_event(event_type: KlineEventType) -> KlineEvent {
    KlineEvent {
        exchange: "binance".into(),
        symbol: "BTC/USDT".into(),
        timeframe: Timeframe::M1,
        candle: Candle {
            open_time: 1700000000000,
            close_time: 1700000059999,
            open: 50000.0,
            high: 50200.0,
            low: 49900.0,
            close: 50100.0,
            volume: 100.5,
            quote_volume: 5025000.0,
            trades: 1500,
            closed: false,
        },
        event_type,
    }
}

fn make_orderbook_event() -> OrderBookEvent {
    OrderBookEvent {
        exchange: "binance".into(),
        symbol: "BTC/USDT".into(),
        bids: vec![
            OrderBookLevel { price: 50000.0, amount: 1.5 },
            OrderBookLevel { price: 49990.0, amount: 2.0 },
        ],
        asks: vec![
            OrderBookLevel { price: 50010.0, amount: 0.8 },
            OrderBookLevel { price: 50020.0, amount: 1.2 },
        ],
        timestamp: 1700000000000,
    }
}
