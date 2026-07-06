//! Integration tests for virs-position — cross-module chain verification.

use chrono::Utc;
use uuid::Uuid;
use virs_position::tracker::PnlTracker;
use virs_types::enums::{PositionSide, PositionStatus, Side, TradeType};
use virs_types::position::{Position, Trade};

#[allow(dead_code)]
fn make_position(
    symbol: &str,
    side: PositionSide,
    size: f64,
    entry_price: f64,
    current_price: f64,
    leverage: u32,
    liquidation_price: Option<f64>,
) -> Position {
    Position {
        id: Uuid::new_v4(),
        engine_id: "test".to_string(),
        strategy_id: None,
        exchange: "binance".to_string(),
        symbol: symbol.to_string(),
        side,
        status: PositionStatus::Open,
        size,
        entry_price,
        current_price,
        leverage,
        margin: size * entry_price / leverage as f64,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss: None,
        take_profit: None,
        liquidation_price,
        opened_at: Utc::now(),
        updated_at: Utc::now(),
        closed_at: None,
        metadata: serde_json::Value::Null,
    }
}

fn make_trade(pnl: f64) -> Trade {
    Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        price: 100.0,
        amount: 1.0,
        fee: 0.1,
        fee_currency: "USDT".to_string(),
        pnl,
        trade_type: TradeType::Close,
        created_at: Utc::now(),
    }
}

// ── INT-5: Tracker record → snapshot chain ─────────────────

#[test]
fn int_5_1_tracker_record_then_snapshot() {
    let mut tracker = PnlTracker::new(10000.0);

    // Record trades
    tracker.record_trade(&make_trade(100.0));
    tracker.record_trade(&make_trade(-50.0));
    tracker.record_trade(&make_trade(200.0));

    let snapshot = tracker.snapshot(0.0);
    // realized = 100 - 50 + 200 = 250, equity = 10000 + 250 = 10250
    assert!((snapshot.equity - 10250.0).abs() < 1e-10);
}
