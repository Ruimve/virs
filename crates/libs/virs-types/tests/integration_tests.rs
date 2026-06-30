//! Integration tests for virs-types.
//!
//! Tests cross-struct computation chains, serde round-trips with method calls,
//! and business logic consistency.

use chrono::Utc;
use uuid::Uuid;

use virs_types::enums::*;
use virs_types::market::*;
use virs_types::position::*;

// ============================================================
// TC-INT-1: Position PnL computation chain
// ============================================================

#[test]
fn int_1_1_long_position_pnl_chain() {
    let pos = make_position(PositionSide::Long, 50000.0, 1.0, 50000.0);
    let pnl = pos.unrealized_pnl_at(51000.0);
    let pct = pos.pnl_pct_at(51000.0);
    assert!((pnl - 1000.0).abs() < 0.01);
    assert!((pct - 2.0).abs() < 0.01);
}

#[test]
fn int_1_2_short_position_pnl_chain() {
    let pos = make_position(PositionSide::Short, 50000.0, 1.0, 50000.0);
    let pnl = pos.unrealized_pnl_at(49000.0);
    assert!((pnl - 1000.0).abs() < 0.01);
}

// ============================================================
// TC-INT-2: Order status judgment chain
// ============================================================

#[test]
fn int_2_1_filled_order_chain() {
    let order = make_order(OrderStatus::Filled, 10.0, 10.0);
    assert!(order.is_filled());
    assert!(!order.is_open());
    assert!((order.fill_rate() - 1.0).abs() < 0.0001);
}

#[test]
fn int_2_2_open_order_chain() {
    let order = make_order(OrderStatus::Open, 0.0, 10.0);
    assert!(!order.is_filled());
    assert!(order.is_open());
    assert!((order.fill_rate() - 0.0).abs() < 0.0001);
}

// ============================================================
// TC-INT-3: ExchangePosition PnL chain
// ============================================================

#[test]
fn int_3_1_exchange_position_pnl_chain() {
    let pos = ExchangePosition {
        symbol: "BTC/USDT".into(), side: PositionSide::Long,
        size: 1.0, entry_price: 50000.0, leverage: 10,
        unrealized_pnl: 0.0, liquidation_price: None,
    };
    assert!((pos.unrealized_pnl_at(51000.0) - 1000.0).abs() < 0.01);
    assert!((pos.pnl_pct_at(51000.0) - 2.0).abs() < 0.01);
    assert!(pos.is_long());
}

// ============================================================
// TC-INT-4: OrderBook derived calculations
// ============================================================
// Removed: OrderBook::best_bid/best_ask/spread/mid_price were orphan methods
// (only used in tests, no business consumer). Tests deleted alongside the methods.

// ============================================================
// TC-INT-5: Ticker derived calculations
// ============================================================
// Removed: Ticker::mid_price/spread were orphan methods (only used in tests,
// no business consumer). Tests deleted alongside the methods.

// ============================================================
// TC-INT-6: RiskConfig validation chain
// ============================================================

#[test]
fn int_6_1_default_config_valid() {
    let config = RiskConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn int_6_2_invalid_config() {
    let mut config = RiskConfig::default();
    config.max_leverage = 0;
    assert!(config.validate().is_err());
}

// ============================================================
// TC-INT-7: Side × PositionSide combinations
// ============================================================

#[test]
fn int_7_1_opening_and_closing_complementary() {
    // For Long: Buy opens, Sell closes
    assert!(Side::Buy.is_opening_for(PositionSide::Long));
    assert!(Side::Sell.is_closing_for(PositionSide::Long));
    assert!(!Side::Buy.is_closing_for(PositionSide::Long));
    assert!(!Side::Sell.is_opening_for(PositionSide::Long));

    // For Short: Sell opens, Buy closes
    assert!(Side::Sell.is_opening_for(PositionSide::Short));
    assert!(Side::Buy.is_closing_for(PositionSide::Short));
    assert!(!Side::Sell.is_closing_for(PositionSide::Short));
    assert!(!Side::Buy.is_opening_for(PositionSide::Short));
}

// ============================================================
// TC-INT-8: serde + method chain
// ============================================================

#[test]
fn int_8_1_exchange_position_serde_then_pnl() {
    let pos = ExchangePosition {
        symbol: "BTC/USDT".into(), side: PositionSide::Long,
        size: 2.0, entry_price: 50000.0, leverage: 10,
        unrealized_pnl: 0.0, liquidation_price: None,
    };
    let original_pnl = pos.unrealized_pnl_at(52000.0);
    let json = serde_json::to_string(&pos).unwrap();
    let de: ExchangePosition = serde_json::from_str(&json).unwrap();
    assert!((de.unrealized_pnl_at(52000.0) - original_pnl).abs() < 0.01);
}

// Removed: int_8_2 used orphan method MarketType::from_str_lossy (no business
// consumer). int_8_3 (AutoMarketType::from_str_lossy) restored below — used by
// virs-app::adapters::auto_store.

#[test]
fn int_8_3_auto_market_type_from_str() {
    use virs_types::auto_port::AutoMarketType;
    assert!(AutoMarketType::from_str_lossy("perpetual").is_perpetual());
    assert!(AutoMarketType::from_str_lossy("spot").is_spot());
}

// ============================================================
// Helpers
// ============================================================

fn make_position(side: PositionSide, entry: f64, size: f64, margin: f64) -> Position {
    Position {
        id: Uuid::nil(), engine_id: "test".into(), strategy_id: None,
        exchange: "binance".into(), symbol: "BTC/USDT".into(),
        side, status: PositionStatus::Open,
        size, entry_price: entry, current_price: entry,
        leverage: 10, margin,
        unrealized_pnl: 0.0, realized_pnl: 0.0,
        stop_loss: None, take_profit: None, liquidation_price: None,
        opened_at: Utc::now(), updated_at: Utc::now(), closed_at: None,
        metadata: serde_json::json!({}),
    }
}

fn make_order(status: OrderStatus, filled: f64, amount: f64) -> PositionOrder {
    PositionOrder {
        id: Uuid::nil(), position_id: Uuid::nil(),
        exchange_order_id: None, client_order_id: None,
        exchange: "binance".into(), symbol: "BTC/USDT".into(),
        side: Side::Buy, order_type: OrderType::Limit,
        request_price: Some(50000.0), fill_price: None,
        amount, filled, remaining: amount - filled,
        status, reduce_only: false,
        fee: 0.0, fee_currency: "USDT".into(), slippage: None,
        created_at: Utc::now(), updated_at: Utc::now(),
    }
}
