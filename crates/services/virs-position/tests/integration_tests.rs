//! Integration tests for virs-position — cross-module chain verification.

use std::collections::HashMap;

use chrono::Utc;
use uuid::Uuid;
use virs_position::risk::{
    calc_symbol_exposure, check_drawdown, check_funding_rate,
    check_liquidation, DrawdownAction, RiskChecker,
};
use virs_position::tracker::{calc_drawdown_pct, calc_unrealized_pnl, PnlTracker};
use virs_types::enums::{PositionSide, PositionStatus, Side, TradeType};
use virs_types::position::{Position, RiskConfig, Trade};

fn make_config() -> RiskConfig {
    RiskConfig {
        max_position_per_symbol_pct: 1.0,
        max_total_position_pct: 3.0,
        max_order_amount_pct: 0.5,
        max_drawdown_pct: 0.2,
        max_leverage: 20,
        funding_rate_threshold: 0.001,
        liquidation_buffer_pct: 0.05,
        max_consecutive_losses: 3,
    }
}

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

// ── INT-1: Funding rate → Drawdown chain ───────────────────

#[test]
fn int_1_1_funding_rate_then_drawdown() {
    let config = make_config();
    // Step 1: Funding rate alert
    let alert = check_funding_rate(&config, "BTC/USDT", 0.003).unwrap();
    assert_eq!(alert.severity, "critical");

    // Step 2: Drawdown check (simulate equity drop after funding payment)
    let action = check_drawdown(&config, 10000.0, 7500.0).unwrap();
    assert_eq!(action, DrawdownAction::CloseAll);
}

#[test]
fn int_1_2_drawdown_escalation() {
    let config = make_config();
    let peak = 10000.0;

    // Normal: drawdown < 50% of max_dd (0.1)
    assert_eq!(check_drawdown(&config, peak, 9500.0), None);

    // Warning: drawdown >= 50% of max_dd
    assert_eq!(check_drawdown(&config, peak, 8800.0), Some(DrawdownAction::Warning));

    // Pause: drawdown >= 75% of max_dd
    assert_eq!(check_drawdown(&config, peak, 8400.0), Some(DrawdownAction::Pause));

    // CloseAll: drawdown >= max_dd
    assert_eq!(check_drawdown(&config, peak, 7500.0), Some(DrawdownAction::CloseAll));
}

// ── INT-2: Exposure → Risk check chain ─────────────────────

#[test]
fn int_2_1_exposure_then_risk_check() {
    let config = make_config();
    let checker = RiskChecker::new(config.clone());

    let pos = make_position("BTC/USDT", PositionSide::Long, 2.0, 100.0, 100.0, 10, None);
    let positions: Vec<&Position> = vec![&pos];

    // Verify exposure calculation
    let exposure = calc_symbol_exposure(&positions, "BTC/USDT");
    assert!((exposure - 20.0).abs() < 1e-10);

    // Risk check should pass (exposure 20 + new 10 = 30 < limit 10000 * 1.0 = 10000)
    let result = checker.check_open_position(&positions, "BTC/USDT", 10.0, 10, 10000.0);
    assert!(result.is_ok());
}

#[test]
fn int_2_2_exposure_limit_reached() {
    let config = make_config();
    let checker = RiskChecker::new(config.clone());

    // max_position_per_symbol_pct = 1.0, equity = 100 → limit = 100
    let pos = make_position("BTC/USDT", PositionSide::Long, 10.0, 100.0, 100.0, 10, None);
    let positions: Vec<&Position> = vec![&pos];
    // existing exposure = 10*100/10 = 100, new margin = 50/10 = 5 → 105 > 100
    let result = checker.check_open_position(&positions, "BTC/USDT", 50.0, 10, 100.0);
    assert!(result.is_err());
}

// ── INT-3: PnL → Drawdown chain ────────────────────────────

#[test]
fn int_3_1_pnl_then_drawdown() {
    let config = make_config();
    let pos1 = make_position("BTC/USDT", PositionSide::Long, 2.0, 100.0, 0.0, 10, None);
    let positions: Vec<&Position> = vec![&pos1];
    let mut prices = HashMap::new();
    prices.insert("BTC/USDT".to_string(), 80.0);

    // Step 1: Calculate unrealized PnL
    let pnl = calc_unrealized_pnl(&positions, &prices);
    // (80 - 100) * 2 = -40
    assert!((pnl - (-40.0)).abs() < 1e-10);

    // Step 2: Calculate drawdown from PnL
    let initial = 1000.0;
    let equity = initial + pnl; // 960
    let drawdown = calc_drawdown_pct(initial, equity);
    // (1000 - 960) / 1000 = 0.04
    assert!((drawdown - 0.04).abs() < 1e-10);

    // Step 3: Check drawdown action
    let action = check_drawdown(&config, initial, equity);
    assert_eq!(action, None); // 0.04 < 0.1 (50% of 0.2)
}

// ── INT-4: Liquidation → Drawdown chain ────────────────────

#[test]
fn int_4_1_liquidation_then_drawdown() {
    let config = make_config();

    // Step 1: Liquidation alert (close to liquidation)
    let pos = make_position("BTC/USDT", PositionSide::Long, 1.0, 100.0, 98.0, 10, Some(96.0));
    let liq_dist = check_liquidation(&config, &pos).unwrap();
    // distance = (98 - 96) / 98 ≈ 0.0204 ≤ 0.05
    assert!(liq_dist <= 0.05);

    // Step 2: Drawdown check (if liquidated, equity drops)
    let action = check_drawdown(&config, 10000.0, 7000.0);
    assert_eq!(action, Some(DrawdownAction::CloseAll));
}

#[test]
fn int_4_2_risk_checker_record_and_check() {
    let config = make_config();
    let mut checker = RiskChecker::new(config);

    // Record 3 consecutive losses
    checker.record_trade_result(-100.0);
    assert!(!checker.should_reduce_position()); // 1 < 3

    checker.record_trade_result(-50.0);
    assert!(!checker.should_reduce_position()); // 2 < 3

    checker.record_trade_result(-30.0);
    assert!(checker.should_reduce_position()); // 3 >= 3

    // A win resets the counter
    checker.record_trade_result(200.0);
    assert!(!checker.should_reduce_position()); // reset to 0
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

// ── INT-6: Boundary value tests ────────────────────────────

#[test]
fn int_6_1_funding_rate_severity_threshold() {
    let config = make_config();
    // threshold = 0.001, critical threshold = 0.002

    // Just below critical (0.0019)
    let alert = check_funding_rate(&config, "BTC/USDT", 0.0019).unwrap();
    assert_eq!(alert.severity, "warning");

    // Exactly at critical (0.002)
    let alert = check_funding_rate(&config, "BTC/USDT", 0.002).unwrap();
    assert_eq!(alert.severity, "critical");
}

#[test]
fn int_6_2_drawdown_boundary_values() {
    let config = make_config();
    let peak = 10000.0;

    // Below warning (drawdown ≈ 0.049 < 0.1 = 0.5 * max_dd)
    assert_eq!(check_drawdown(&config, peak, 9500.0), None);

    // At warning level (drawdown ≈ 0.12, in [0.1, 0.15))
    assert_eq!(check_drawdown(&config, peak, 8800.0), Some(DrawdownAction::Warning));

    // At pause level (drawdown ≈ 0.16, in [0.15, 0.2))
    assert_eq!(check_drawdown(&config, peak, 8400.0), Some(DrawdownAction::Pause));

    // At close_all level (drawdown = 0.25 >= max_dd)
    assert_eq!(check_drawdown(&config, peak, 7500.0), Some(DrawdownAction::CloseAll));
}
