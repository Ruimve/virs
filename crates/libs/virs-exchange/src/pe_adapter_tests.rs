//! Unit tests for pe_adapter.rs conversion functions.

use chrono::Utc;

use virs_models as models;
use virs_types::enums::*;
use virs_types::market::ExchangePosition;
use virs_types::position::{PositionEngineError, PositionOrder, WsFeedEvent};

use crate::pe_adapter::{
    convert_exchange_position, convert_order, convert_order_status, convert_order_type,
    convert_position_side, convert_side, convert_to_models_side, convert_virs_market_type,
    convert_virs_position_side, convert_ws_feed_event, no_exchange_error, to_pe_error,
};

// ============================================================
// TC-PE1: convert_side
// ============================================================

#[test]
fn pe1_1_buy() {
    assert_eq!(convert_side(&models::Side::Buy), Side::Buy);
}

#[test]
fn pe1_2_sell() {
    assert_eq!(convert_side(&models::Side::Sell), Side::Sell);
}

// ============================================================
// TC-PE2: convert_to_models_side
// ============================================================

#[test]
fn pe2_1_buy() {
    assert_eq!(convert_to_models_side(&Side::Buy), models::Side::Buy);
}

#[test]
fn pe2_2_sell() {
    assert_eq!(convert_to_models_side(&Side::Sell), models::Side::Sell);
}

// ============================================================
// TC-PE3: convert_position_side (PE PositionSide → models PositionSide)
// ============================================================

#[test]
fn pe3_1_long() {
    assert_eq!(
        convert_position_side(&Some(PositionSide::Long)),
        Some(models::PositionSide::Long)
    );
}

#[test]
fn pe3_2_short() {
    assert_eq!(
        convert_position_side(&Some(PositionSide::Short)),
        Some(models::PositionSide::Short)
    );
}

#[test]
fn pe3_3_both() {
    assert_eq!(
        convert_position_side(&Some(PositionSide::Both)),
        Some(models::PositionSide::Both)
    );
}

#[test]
fn pe3_4_none() {
    assert_eq!(convert_position_side(&None), None);
}

// ============================================================
// TC-PE4: convert_order_type (PE OrderType → models OrderType)
// ============================================================

#[test]
fn pe4_1_market() {
    assert_eq!(
        convert_order_type(&OrderType::Market),
        models::OrderType::Market
    );
}

#[test]
fn pe4_2_limit() {
    assert_eq!(
        convert_order_type(&OrderType::Limit),
        models::OrderType::Limit
    );
}

#[test]
fn pe4_3_stop_market() {
    assert_eq!(
        convert_order_type(&OrderType::StopMarket),
        models::OrderType::StopMarket
    );
}

#[test]
fn pe4_4_stop_limit() {
    assert_eq!(
        convert_order_type(&OrderType::StopLimit),
        models::OrderType::StopLimit
    );
}

#[test]
fn pe4_5_take_profit_market() {
    assert_eq!(
        convert_order_type(&OrderType::TakeProfitMarket),
        models::OrderType::TakeProfitMarket
    );
}

// ============================================================
// TC-PE5: convert_order_status
// ============================================================

#[test]
fn pe5_1_open() {
    assert_eq!(
        convert_order_status(&models::OrderStatus::Open),
        OrderStatus::Open
    );
}

#[test]
fn pe5_2_partially_filled() {
    assert_eq!(
        convert_order_status(&models::OrderStatus::PartiallyFilled),
        OrderStatus::PartiallyFilled
    );
}

#[test]
fn pe5_3_filled() {
    assert_eq!(
        convert_order_status(&models::OrderStatus::Filled),
        OrderStatus::Filled
    );
}

#[test]
fn pe5_4_canceled() {
    assert_eq!(
        convert_order_status(&models::OrderStatus::Canceled),
        OrderStatus::Canceled
    );
}

#[test]
fn pe5_5_failed() {
    assert_eq!(
        convert_order_status(&models::OrderStatus::Failed),
        OrderStatus::Failed
    );
}

#[test]
fn pe5_6_pending() {
    assert_eq!(
        convert_order_status(&models::OrderStatus::Pending),
        OrderStatus::Pending
    );
}

// ============================================================
// TC-PE6: convert_virs_position_side
// ============================================================

#[test]
fn pe6_1_long() {
    assert_eq!(
        convert_virs_position_side(&models::PositionSide::Long),
        PositionSide::Long
    );
}

#[test]
fn pe6_2_short() {
    assert_eq!(
        convert_virs_position_side(&models::PositionSide::Short),
        PositionSide::Short
    );
}

#[test]
fn pe6_3_both() {
    assert_eq!(
        convert_virs_position_side(&models::PositionSide::Both),
        PositionSide::Both
    );
}

// ============================================================
// TC-PE7: convert_virs_market_type
// ============================================================

#[test]
fn pe7_1_spot() {
    assert_eq!(
        convert_virs_market_type(&models::MarketType::Spot),
        MarketType::Spot
    );
}

#[test]
fn pe7_2_perpetual() {
    assert_eq!(
        convert_virs_market_type(&models::MarketType::Perpetual),
        MarketType::Perpetual
    );
}

// ============================================================
// TC-PE8: convert_order
// ============================================================

#[test]
fn pe8_1_order_normal() {
    let now = Utc::now();
    let mo = models::Order {
        id: "order_123".into(),
        client_order_id: Some("client_456".into()),
        symbol: "BTC/USDT".into(),
        side: models::Side::Buy,
        order_type: models::OrderType::Limit,
        price: Some(50000.0),
        amount: 1.0,
        cost: Some(50000.0),
        filled: 0.5,
        remaining: 0.5,
        status: models::OrderStatus::PartiallyFilled,
        fee: 0.075,
        fee_currency: "BTC".into(),
        created_at: now,
        updated_at: now,
    };
    let po: PositionOrder = convert_order(&mo, "binance");
    assert_eq!(po.exchange_order_id, Some("order_123".into()));
    assert_eq!(po.client_order_id, Some("client_456".into()));
    assert_eq!(po.exchange, "binance");
    assert_eq!(po.symbol, "BTC/USDT");
    assert_eq!(po.side, Side::Buy);
    assert_eq!(po.order_type, OrderType::Limit);
    assert_eq!(po.request_price, Some(50000.0));
    assert_eq!(po.fill_price, Some(50000.0)); // filled > 0 → Some(price)
    assert_eq!(po.amount, 1.0);
    assert_eq!(po.filled, 0.5);
    assert_eq!(po.remaining, 0.5);
    assert_eq!(po.status, OrderStatus::PartiallyFilled);
    assert_eq!(po.fee, 0.075);
    assert_eq!(po.fee_currency, "BTC");
    assert_eq!(po.created_at, now);
    assert_eq!(po.updated_at, now);
}

#[test]
fn pe8_2_order_no_fill_no_price() {
    let now = Utc::now();
    let mo = models::Order {
        id: "order_789".into(),
        client_order_id: None,
        symbol: "ETH/USDT".into(),
        side: models::Side::Sell,
        order_type: models::OrderType::Market,
        price: None,
        amount: 2.0,
        cost: None,
        filled: 0.0,
        remaining: 2.0,
        status: models::OrderStatus::Open,
        fee: 0.0,
        fee_currency: "".into(),
        created_at: now,
        updated_at: now,
    };
    let po = convert_order(&mo, "binance");
    assert_eq!(po.client_order_id, None);
    assert_eq!(po.request_price, None);
    assert_eq!(po.fill_price, None); // filled == 0 → None
    assert_eq!(po.status, OrderStatus::Open);
}

// ============================================================
// TC-PE9: convert_exchange_position
// ============================================================

#[test]
fn pe9_1_position_normal() {
    let ep = models::ExchangePosition {
        symbol: "BTC/USDT".into(),
        side: models::PositionSide::Long,
        size: 1.0,
        entry_price: 50000.0,
        leverage: 10,
        unrealized_pnl: 500.0,
        liquidation_price: Some(45000.0),
    };
    let result: ExchangePosition = convert_exchange_position(&ep);
    assert_eq!(result.symbol, "BTC/USDT");
    assert_eq!(result.side, PositionSide::Long);
    assert_eq!(result.size, 1.0);
    assert_eq!(result.entry_price, 50000.0);
    assert_eq!(result.leverage, 10);
    assert_eq!(result.unrealized_pnl, 500.0);
    assert_eq!(result.liquidation_price, Some(45000.0));
}

#[test]
fn pe9_2_position_no_liquidation() {
    let ep = models::ExchangePosition {
        symbol: "ETH/USDT".into(),
        side: models::PositionSide::Short,
        size: 5.0,
        entry_price: 3000.0,
        leverage: 5,
        unrealized_pnl: -100.0,
        liquidation_price: None,
    };
    let result = convert_exchange_position(&ep);
    assert_eq!(result.liquidation_price, None);
    assert_eq!(result.side, PositionSide::Short);
}

// ============================================================
// TC-PE10: convert_ws_feed_event
// ============================================================

#[test]
fn pe10_1_order_update() {
    let now = Utc::now();
    let event = virs_ccxt::WsFeedEvent::OrderUpdate {
        exchange_order_id: "order_1".into(),
        symbol: "BTC/USDT".into(),
        status: virs_types::OrderStatus::Filled,
        filled: 1.0,
        remaining: 0.0,
        price: 50000.0,
        amount: 1.0,
        commission: 0.05,
        timestamp: now,
        position_side: Some(virs_types::PositionSide::Long),
    };
    let result: WsFeedEvent = convert_ws_feed_event(event);
    match result {
        WsFeedEvent::OrderUpdate {
            exchange_order_id, symbol, status, filled, remaining, price, amount, commission, timestamp, position_side,
        } => {
            assert_eq!(exchange_order_id, "order_1");
            assert_eq!(symbol, "BTC/USDT");
            assert_eq!(status, OrderStatus::Filled);
            assert_eq!(filled, 1.0);
            assert_eq!(remaining, 0.0);
            assert_eq!(price, 50000.0);
            assert_eq!(amount, 1.0);
            assert_eq!(commission, 0.05);
            assert_eq!(timestamp, now);
            assert_eq!(position_side, Some(PositionSide::Long));
        }
        _ => panic!("Expected OrderUpdate"),
    }
}

#[test]
fn pe10_2_connection_changed() {
    let event = virs_ccxt::WsFeedEvent::ConnectionChanged { connected: true };
    let result: WsFeedEvent = convert_ws_feed_event(event);
    assert_eq!(result, WsFeedEvent::ConnectionChanged { connected: true });
}

// ============================================================
// TC-PE11: to_pe_error
// ============================================================

#[test]
fn pe11_1_network_error() {
    let err = anyhow::anyhow!("connection timeout");
    let pe_err = to_pe_error(err);
    match pe_err {
        PositionEngineError::Exchange(msg) => {
            assert!(msg.contains("connection timeout"));
        }
        _ => panic!("Expected Exchange variant"),
    }
}

#[test]
fn pe11_2_auth_error() {
    let err = anyhow::anyhow!("invalid api key");
    let pe_err = to_pe_error(err);
    match pe_err {
        PositionEngineError::Exchange(msg) => {
            assert!(msg.contains("invalid api key"));
        }
        _ => panic!("Expected Exchange variant"),
    }
}

// ============================================================
// TC-PE12: no_exchange_error
// ============================================================

#[test]
fn pe12_1_no_exchange_error() {
    let err = no_exchange_error();
    match err {
        PositionEngineError::Exchange(msg) => {
            assert!(msg.contains("No perpetual exchange"));
        }
        _ => panic!("Expected Exchange variant"),
    }
}
