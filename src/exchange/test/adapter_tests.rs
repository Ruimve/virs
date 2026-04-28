use chrono::Utc;
use uuid::Uuid;

use crate::exchange::binance_position_adapter::*;
use crate::models;
use crate::position::types::*;

// ============================================================
// 类型转换 (6 tests)
// ============================================================

#[test]
fn test_convert_side_buy() {
    let result = convert_side(&models::Side::Buy);
    assert_eq!(result, Side::Buy);
}

#[test]
fn test_convert_side_sell() {
    let result = convert_side(&models::Side::Sell);
    assert_eq!(result, Side::Sell);
}

#[test]
fn test_convert_position_side_long() {
    let result = convert_virs_position_side(&models::PositionSide::Long);
    assert_eq!(result, PositionSide::Long);
}

#[test]
fn test_convert_position_side_short() {
    let result = convert_virs_position_side(&models::PositionSide::Short);
    assert_eq!(result, PositionSide::Short);
}

#[test]
fn test_convert_order_status_all_variants() {
    assert_eq!(convert_order_status(&models::OrderStatus::Pending), OrderStatus::Pending);
    assert_eq!(convert_order_status(&models::OrderStatus::Open), OrderStatus::Open);
    assert_eq!(convert_order_status(&models::OrderStatus::PartiallyFilled), OrderStatus::PartiallyFilled);
    assert_eq!(convert_order_status(&models::OrderStatus::Filled), OrderStatus::Filled);
    assert_eq!(convert_order_status(&models::OrderStatus::Canceled), OrderStatus::Canceled);
    assert_eq!(convert_order_status(&models::OrderStatus::Failed), OrderStatus::Failed);
}

#[test]
fn test_convert_order_type_all_variants() {
    // convert_order_type: PE::OrderType -> models::OrderType
    assert_eq!(convert_order_type(&OrderType::Limit), models::OrderType::Limit);
    assert_eq!(convert_order_type(&OrderType::Market), models::OrderType::Market);
    assert_eq!(convert_order_type(&OrderType::StopMarket), models::OrderType::StopMarket);
    assert_eq!(convert_order_type(&OrderType::TakeProfitMarket), models::OrderType::StopMarket);
}

// ============================================================
// Order 转换 (4 tests)
// ============================================================

/// 辅助函数：创建一个基础的 models::Order 用于测试
fn make_virs_order() -> models::Order {
    models::Order {
        id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        client_order_id: Some("client-123".to_string()),
        symbol: "BTC/USDT:USDT".to_string(),
        side: models::Side::Buy,
        order_type: models::OrderType::Limit,
        price: Some(50000.0),
        amount: 0.1,
        cost: None,
        filled: 0.05,
        remaining: 0.05,
        status: models::OrderStatus::PartiallyFilled,
        fee: 0.001,
        fee_currency: "USDT".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[test]
fn test_convert_order_basic_fields() {
    let virs_order = make_virs_order();
    let pe_order = convert_order(&virs_order);

    assert_eq!(pe_order.symbol, "BTC/USDT:USDT");
    assert_eq!(pe_order.side, Side::Buy);
    assert_eq!(pe_order.order_type, OrderType::Limit);
    assert_eq!(pe_order.amount, 0.1);
    assert_eq!(pe_order.filled, 0.05);
    assert_eq!(pe_order.remaining, 0.05);
    assert_eq!(pe_order.status, OrderStatus::PartiallyFilled);
}

#[test]
fn test_convert_order_price_fields() {
    // filled > 0 时 fill_price = Some(price)
    let mut virs_order = make_virs_order();
    virs_order.filled = 0.05;
    virs_order.price = Some(50000.0);
    let pe_order = convert_order(&virs_order);
    assert_eq!(pe_order.request_price, Some(50000.0));
    assert_eq!(pe_order.fill_price, Some(50000.0));

    // filled == 0 时 fill_price = None
    virs_order.filled = 0.0;
    let pe_order = convert_order(&virs_order);
    assert_eq!(pe_order.request_price, Some(50000.0));
    assert_eq!(pe_order.fill_price, None);

    // price = None 时 request_price = None, fill_price = None
    virs_order.price = None;
    let pe_order = convert_order(&virs_order);
    assert_eq!(pe_order.request_price, None);
    assert_eq!(pe_order.fill_price, None);
}

#[test]
fn test_convert_order_fee_fields() {
    let virs_order = make_virs_order();
    let pe_order = convert_order(&virs_order);

    assert_eq!(pe_order.fee, 0.001);
    assert_eq!(pe_order.fee_currency, "USDT");

    // 零手续费
    let mut virs_order = make_virs_order();
    virs_order.fee = 0.0;
    virs_order.fee_currency = String::new();
    let pe_order = convert_order(&virs_order);
    assert_eq!(pe_order.fee, 0.0);
    assert_eq!(pe_order.fee_currency, "");
}

#[test]
fn test_convert_order_id_handling() {
    let virs_order = make_virs_order();
    let pe_order = convert_order(&virs_order);

    // models::Order.id (String) -> PE::Order.id (Uuid)
    // 使用有效的 UUID 字符串时应该正确解析
    assert_eq!(
        pe_order.id,
        Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
    );

    // exchange_order_id 应该保留原始字符串
    assert_eq!(pe_order.exchange_order_id, Some("550e8400-e29b-41d4-a716-446655440000".to_string()));

    // position_id 应该是 nil（需要调用方设置）
    assert_eq!(pe_order.position_id, Uuid::nil());

    // client_order_id 正确传递
    assert_eq!(pe_order.client_order_id, Some("client-123".to_string()));

    // exchange 初始为空字符串
    assert_eq!(pe_order.exchange, "");

    // reduce_only 默认为 false
    assert!(!pe_order.reduce_only);

    // slippage 默认为 None
    assert_eq!(pe_order.slippage, None);

    // 无效 UUID 字符串时应该 fallback 到 new_v4
    let mut virs_order = make_virs_order();
    virs_order.id = "not-a-uuid".to_string();
    let pe_order = convert_order(&virs_order);
    // 不应该是解析后的 nil UUID，而应该是一个新生成的 v4 UUID
    assert_ne!(pe_order.id, Uuid::nil());
}

// ============================================================
// ExchangePosition 转换 (2 tests)
// ============================================================

#[test]
fn test_convert_exchange_position_basic() {
    let virs_pos = models::ExchangePosition {
        symbol: "BTC/USDT:USDT".to_string(),
        side: models::PositionSide::Long,
        size: 0.5,
        entry_price: 48000.0,
        leverage: 10,
        unrealized_pnl: 100.0,
        liquidation_price: Some(42000.0),
    };

    let pe_pos = convert_exchange_position(&virs_pos);

    assert_eq!(pe_pos.symbol, "BTC/USDT:USDT");
    assert_eq!(pe_pos.side, PositionSide::Long);
    assert_eq!(pe_pos.size, 0.5);
    assert_eq!(pe_pos.entry_price, 48000.0);
    assert_eq!(pe_pos.leverage, 10);
    assert_eq!(pe_pos.unrealized_pnl, 100.0);
}

#[test]
fn test_convert_exchange_position_liquidation_price() {
    // Some -> Some
    let virs_pos = models::ExchangePosition {
        symbol: "BTC/USDT:USDT".to_string(),
        side: models::PositionSide::Short,
        size: 1.0,
        entry_price: 50000.0,
        leverage: 5,
        unrealized_pnl: -50.0,
        liquidation_price: Some(55000.0),
    };
    let pe_pos = convert_exchange_position(&virs_pos);
    assert_eq!(pe_pos.liquidation_price, Some(55000.0));

    // None -> None
    let mut virs_pos = virs_pos;
    virs_pos.liquidation_price = None;
    let pe_pos = convert_exchange_position(&virs_pos);
    assert_eq!(pe_pos.liquidation_price, None);
}

// ============================================================
// Ticker 转换 (1 test)
// ============================================================

#[test]
fn test_convert_ticker_fields() {
    let now = Utc::now();
    let virs_ticker = models::Ticker {
        symbol: "BTC/USDT:USDT".to_string(),
        exchange: "binance".to_string(),
        bid: 49990.0,
        ask: 50010.0,
        last: 50000.0,
        high_24h: 51000.0,
        low_24h: 49000.0,
        volume_24h: 12345.6,
        price_change_24h: 500.0,
        price_change_pct_24h: 1.0,
        timestamp: now,
    };

    let pe_ticker = convert_ticker(&virs_ticker);

    assert_eq!(pe_ticker.symbol, "BTC/USDT:USDT");
    // price = last
    assert_eq!(pe_ticker.price, 50000.0);
    assert_eq!(pe_ticker.bid, 49990.0);
    assert_eq!(pe_ticker.ask, 50010.0);
    assert_eq!(pe_ticker.volume_24h, 12345.6);
    assert_eq!(pe_ticker.timestamp, now);
}

// ============================================================
// FundingRate 转换 (2 tests)
// ============================================================

#[test]
fn test_convert_funding_rate_with_time() {
    let funding_time = Utc::now();
    let virs_fr = models::FundingRate {
        symbol: "BTC/USDT:USDT".to_string(),
        rate: 0.0001,
        next_funding_time: Some(funding_time),
    };

    let pe_fr = convert_funding_rate(&virs_fr);

    assert_eq!(pe_fr.symbol, "BTC/USDT:USDT");
    assert_eq!(pe_fr.rate, 0.0001);
    assert_eq!(pe_fr.next_funding_time, funding_time);
}

#[test]
fn test_convert_funding_rate_without_time() {
    let before = Utc::now();
    let virs_fr = models::FundingRate {
        symbol: "BTC/USDT:USDT".to_string(),
        rate: 0.0002,
        next_funding_time: None,
    };

    let pe_fr = convert_funding_rate(&virs_fr);
    let after = Utc::now();

    assert_eq!(pe_fr.symbol, "BTC/USDT:USDT");
    assert_eq!(pe_fr.rate, 0.0002);
    // next_funding_time 应该在 before 和 after 之间（使用 Utc::now() 作为 fallback）
    assert!(pe_fr.next_funding_time >= before && pe_fr.next_funding_time <= after);
}
