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
    assert!((pnl - 1000.0).abs() < 0.01);
}

#[test]
fn int_1_2_short_position_pnl_chain() {
    let pos = make_position(PositionSide::Short, 50000.0, 1.0, 50000.0);
    let pnl = pos.unrealized_pnl_at(49000.0);
    assert!((pnl - 1000.0).abs() < 0.01);
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

#[test]
fn int_8_3_auto_market_type_from_str() {
    use virs_types::auto_port::AutoMarketType;
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
