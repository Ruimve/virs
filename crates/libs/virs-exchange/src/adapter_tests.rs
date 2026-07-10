//! Unit tests for adapter.rs type conversion functions.

use chrono::Utc;

use virs_ccxt::{CcxtKline, CcxtOrder, CcxtOrderStatus, OrderFee};
use virs_models::Order;
use virs_types::enums::*;
use virs_types::market::{Balance, Kline};

use crate::adapter::{
    to_ccxt_market_type, to_ccxt_order_type, to_ccxt_side, to_models_balance, to_models_kline,
    to_models_order,
};

// ============================================================
// TC-A1: to_ccxt_market_type
// ============================================================

#[test]
fn a1_2_perpetual_to_ccxt() {
    assert_eq!(
        to_ccxt_market_type(&MarketType::Perpetual),
        MarketType::Perpetual
    );
}

// ============================================================
// TC-A2: to_ccxt_side
// ============================================================

#[test]
fn a2_1_buy_to_ccxt() {
    assert_eq!(to_ccxt_side(&Side::Buy), virs_ccxt::Side::Buy);
}

#[test]
fn a2_2_sell_to_ccxt() {
    assert_eq!(to_ccxt_side(&Side::Sell), virs_ccxt::Side::Sell);
}

// ============================================================
// TC-A3: to_ccxt_order_type
// ============================================================

#[test]
fn a3_1_market_to_ccxt() {
    assert_eq!(to_ccxt_order_type(&OrderType::Market), virs_ccxt::OrderType::Market);
}

#[test]
fn a3_2_limit_to_ccxt() {
    assert_eq!(to_ccxt_order_type(&OrderType::Limit), virs_ccxt::OrderType::Limit);
}

#[test]
fn a3_3_stop_market_to_ccxt() {
    assert_eq!(to_ccxt_order_type(&OrderType::StopMarket), virs_ccxt::OrderType::StopMarket);
}

#[test]
fn a3_4_stop_limit_to_ccxt() {
    assert_eq!(to_ccxt_order_type(&OrderType::StopLimit), virs_ccxt::OrderType::StopLimit);
}

#[test]
fn a3_5_take_profit_market_to_ccxt() {
    assert_eq!(
        to_ccxt_order_type(&OrderType::TakeProfitMarket),
        virs_ccxt::OrderType::TakeProfitMarket
    );
}

// ============================================================
// TC-A4: to_models_kline
// ============================================================

#[test]
fn a4_1_kline_normal_conversion() {
    let ck = CcxtKline {
        timestamp: 1700000000000,
        close_time: None,
        open: 50000.0,
        high: 51000.0,
        low: 49000.0,
        close: 50500.0,
        volume: 1000.0,
        quote_volume: Some(50000000.0),
        trades: Some(5000),
    };
    let kline: Kline = to_models_kline(ck, "BTC/USDT", "binance", "1m");
    assert_eq!(kline.open_time, 1700000000000);
    assert_eq!(kline.open, 50000.0);
    assert_eq!(kline.high, 51000.0);
    assert_eq!(kline.low, 49000.0);
    assert_eq!(kline.close, 50500.0);
    assert_eq!(kline.volume, 1000.0);
    assert_eq!(kline.quote_volume, 50000000.0);
    assert_eq!(kline.trades, 5000);
    assert_eq!(kline.symbol, "BTC/USDT");
    assert_eq!(kline.exchange, "binance");
    assert_eq!(kline.interval, "1m");
    // close_time = timestamp + 60000 - 1 (1m interval, Binance format: open + tf - 1)
    assert_eq!(kline.close_time, 1700000000000 + 60_000 - 1);
}

#[test]
fn a4_2_kline_exchange_field() {
    let ck = CcxtKline {
        timestamp: 100,
        close_time: None,
        open: 1.0, high: 2.0, low: 0.5, close: 1.5,
        volume: 10.0, quote_volume: None, trades: None,
    };
    let kline = to_models_kline(ck, "ETH/USDC", "okx", "1h");
    assert_eq!(kline.exchange, "okx");
    assert_eq!(kline.symbol, "ETH/USDC");
    assert_eq!(kline.interval, "1h");
    // None → 0.0
    assert_eq!(kline.quote_volume, 0.0);
    assert_eq!(kline.trades, 0);
    // close_time = timestamp + 3_600_000 - 1 (1h interval, Binance format: open + tf - 1)
    assert_eq!(kline.close_time, 100 + 3_600_000 - 1);
}

#[test]
fn a4_3_kline_close_time_binance_format() {
    // T3: close_time must be open_time + interval_ms - 1 (Binance official format)
    // e.g. 1m kline: open=12:00:00.000, close=12:00:59.999
    let intervals: &[(&str, i64)] = &[
        ("1m", 60_000),
        ("5m", 300_000),
        ("15m", 900_000),
        ("30m", 1_800_000),
        ("1h", 3_600_000),
        ("4h", 14_400_000),
        ("1d", 86_400_000),
        ("1w", 604_800_000),
    ];
    for (interval, tf_ms) in intervals {
        let ck = CcxtKline {
            timestamp: 1700000000000,
            close_time: None,
            open: 50000.0,
            high: 51000.0,
            low: 49000.0,
            close: 50500.0,
            volume: 1000.0,
            quote_volume: Some(50000000.0),
            trades: Some(5000),
        };
        let kline = to_models_kline(ck, "BTC/USDT", "binance", interval);
        assert_eq!(
            kline.close_time,
            1700000000000 + tf_ms - 1,
            "close_time mismatch for interval {}",
            interval
        );
        // Verify the invariant: close_time - open_time = interval_ms - 1
        assert_eq!(
            kline.close_time - kline.open_time,
            tf_ms - 1,
            "close_time - open_time must equal interval_ms - 1 for interval {}",
            interval
        );
    }
}

#[test]
fn a4_4_kline_close_time_from_exchange() {
    // T4: When exchange provides close_time (a[6]), use it directly
    let ck = CcxtKline {
        timestamp: 1700000000000,
        close_time: Some(1700000059000), // Exchange-provided close_time (different from computed)
        open: 50000.0,
        high: 51000.0,
        low: 49000.0,
        close: 50500.0,
        volume: 1000.0,
        quote_volume: Some(50000000.0),
        trades: Some(5000),
    };
    let kline = to_models_kline(ck, "BTC/USDT", "binance", "1m");
    // Should use the exchange-provided close_time, not the computed one
    assert_eq!(kline.close_time, 1700000059000);
    assert_ne!(kline.close_time, 1700000000000 + 60_000 - 1);
}

// ============================================================
// TC-A5: to_models_balance
// ============================================================

#[test]
fn a5_1_balance_normal() {
    let cb = Balance {
        asset: "USDT".into(),
        free: 10000.0,
        used: 5000.0,
        total: 15000.0,
    };
    let balance = to_models_balance(cb);
    assert_eq!(balance.asset, "USDT");
    assert_eq!(balance.free, 10000.0);
    assert_eq!(balance.used, 5000.0);
    assert_eq!(balance.total, 15000.0);
}

// ============================================================
// TC-A6: to_models_order
// ============================================================

#[test]
fn a6_1_order_normal() {
    let now = Utc::now();
    let co = CcxtOrder {
        id: "order_123".into(),
        client_order_id: Some("client_456".into()),
        symbol: "BTC/USDT".into(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        price: Some(50000.0),
        amount: 1.0,
        cost: Some(50000.0),
        filled: 0.5,
        remaining: 0.5,
        status: CcxtOrderStatus::PartiallyFilled,
        fee: Some(OrderFee {
            cost: 0.075,
            currency: "BTC".into(),
            rate: Some(0.001),
        }),
        created_at: Some(now),
        updated_at: Some(now),
        info: serde_json::json!({}),
    };
    let order: Order = to_models_order(co);
    assert_eq!(order.id, "order_123");
    assert_eq!(order.client_order_id, Some("client_456".into()));
    assert_eq!(order.symbol, "BTC/USDT");
    assert_eq!(order.side, Side::Buy);
    assert_eq!(order.order_type, OrderType::Limit);
    assert_eq!(order.price, Some(50000.0));
    assert_eq!(order.amount, 1.0);
    assert_eq!(order.cost, Some(50000.0));
    assert_eq!(order.filled, 0.5);
    assert_eq!(order.remaining, 0.5);
    assert_eq!(order.status, OrderStatus::PartiallyFilled);
    assert_eq!(order.fee, 0.075);
    assert_eq!(order.fee_currency, "BTC");
    assert_eq!(order.created_at, now);
    assert_eq!(order.updated_at, now);
}

#[test]
fn a6_2_order_optional_fields_none() {
    let co = CcxtOrder {
        id: "order_789".into(),
        client_order_id: None,
        symbol: "ETH/USDT".into(),
        side: Side::Sell,
        order_type: OrderType::Market,
        price: None,
        amount: 2.0,
        cost: None,
        filled: 2.0,
        remaining: 0.0,
        status: CcxtOrderStatus::Filled,
        fee: None,
        created_at: None,
        updated_at: None,
        info: serde_json::json!({}),
    };
    let order: Order = to_models_order(co);
    assert_eq!(order.client_order_id, None);
    assert_eq!(order.price, None);
    assert_eq!(order.cost, None);
    assert_eq!(order.fee, 0.0);
    assert_eq!(order.fee_currency, "");
    assert_eq!(order.status, OrderStatus::Filled);
}
