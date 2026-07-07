//! Integration tests for virs-position — cross-module chain verification.

use chrono::Utc;
use uuid::Uuid;
use virs_position::tracker::PnlTracker;
use virs_types::enums::{Side, TradeType};
use virs_types::position::Trade;

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
