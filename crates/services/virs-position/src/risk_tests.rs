//! Unit tests for risk.rs idempotent functions

use crate::risk::{
    calc_symbol_exposure, calc_total_exposure, check_drawdown, check_funding_rate,
    check_liquidation, DrawdownAction,
};
use chrono::Utc;
use uuid::Uuid;
use virs_types::enums::{PositionSide, PositionStatus};
use virs_types::position::{Position, RiskConfig};

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

// ── check_funding_rate ─────────────────────────────────────

#[test]
fn r1_1_check_funding_rate_normal() {
    let config = make_config();
    let result = check_funding_rate(&config, "BTC/USDT", 0.0005);
    assert!(result.is_none());
}

#[test]
fn r1_2_check_funding_rate_warning() {
    let config = make_config();
    let result = check_funding_rate(&config, "BTC/USDT", 0.0015);
    assert!(result.is_some());
    let alert = result.unwrap();
    assert_eq!(alert.severity, "warning");
    assert_eq!(alert.alert_type, "funding_rate");
}

#[test]
fn r1_3_check_funding_rate_critical() {
    let config = make_config();
    // threshold * 2 = 0.002, rate = 0.003 > 0.002
    let result = check_funding_rate(&config, "BTC/USDT", 0.003);
    assert!(result.is_some());
    let alert = result.unwrap();
    assert_eq!(alert.severity, "critical");
}

#[test]
fn r1_4_check_funding_rate_negative() {
    let config = make_config();
    // |rate| = 0.0015 > threshold 0.001
    let result = check_funding_rate(&config, "BTC/USDT", -0.0015);
    assert!(result.is_some());
    let alert = result.unwrap();
    assert_eq!(alert.severity, "warning");
}

// ── check_drawdown ─────────────────────────────────────────

#[test]
fn r2_1_check_drawdown_normal() {
    let config = make_config();
    // max_dd = 0.2, drawdown = (100 - 95) / 100 = 0.05 < 0.1 (0.5 * 0.2)
    let result = check_drawdown(&config, 100.0, 95.0);
    assert_eq!(result, None);
}

#[test]
fn r2_2_check_drawdown_warning() {
    let config = make_config();
    // drawdown = 0.12 >= 0.1 (0.5 * 0.2), < 0.15 (0.75 * 0.2)
    let result = check_drawdown(&config, 100.0, 88.0);
    assert_eq!(result, Some(DrawdownAction::Warning));
}

#[test]
fn r2_3_check_drawdown_pause() {
    let config = make_config();
    // drawdown = 0.16 >= 0.15 (0.75 * 0.2), < 0.2
    let result = check_drawdown(&config, 100.0, 84.0);
    assert_eq!(result, Some(DrawdownAction::Pause));
}

#[test]
fn r2_4_check_drawdown_close_all() {
    let config = make_config();
    // drawdown = 0.25 >= 0.2 (max_dd)
    let result = check_drawdown(&config, 100.0, 75.0);
    assert_eq!(result, Some(DrawdownAction::CloseAll));
}

#[test]
fn r2_5_check_drawdown_zero_peak() {
    let config = make_config();
    let result = check_drawdown(&config, 0.0, 50.0);
    assert_eq!(result, None);
}

// ── check_liquidation ──────────────────────────────────────

#[test]
fn r3_1_check_liquidation_none() {
    let config = make_config();
    let pos = make_position("BTC/USDT", PositionSide::Long, 1.0, 100.0, 105.0, 10, None);
    let result = check_liquidation(&config, &pos);
    assert!(result.is_none());
}

#[test]
fn r3_2_check_liquidation_far() {
    let config = make_config();
    // buffer = 0.05, distance = (100 - 90) / 100 = 0.1 > 0.05
    let pos = make_position("BTC/USDT", PositionSide::Long, 1.0, 100.0, 100.0, 10, Some(90.0));
    let result = check_liquidation(&config, &pos);
    assert!(result.is_none());
}

#[test]
fn r3_3_check_liquidation_close() {
    let config = make_config();
    // buffer = 0.05, distance = (100 - 97) / 100 = 0.03 <= 0.05
    let pos = make_position("BTC/USDT", PositionSide::Long, 1.0, 100.0, 100.0, 10, Some(97.0));
    let result = check_liquidation(&config, &pos);
    assert!(result.is_some());
    let pct = result.unwrap();
    assert!((pct - 0.03).abs() < 1e-10);
}

#[test]
fn r3_4_check_liquidation_zero_price() {
    let config = make_config();
    let pos = make_position("BTC/USDT", PositionSide::Long, 1.0, 100.0, 0.0, 10, Some(90.0));
    let result = check_liquidation(&config, &pos);
    assert!(result.is_none());
}

// ── calc_symbol_exposure ───────────────────────────────────

#[test]
fn r4_1_calc_symbol_exposure_empty() {
    let positions: Vec<&Position> = vec![];
    let result = calc_symbol_exposure(&positions, "BTC/USDT");
    assert!((result - 0.0).abs() < 1e-10);
}

#[test]
fn r4_2_calc_symbol_exposure_single() {
    let pos = make_position("BTC/USDT", PositionSide::Long, 2.0, 100.0, 100.0, 10, None);
    // margin = 2 * 100 / 10 = 20
    let positions: Vec<&Position> = vec![&pos];
    let result = calc_symbol_exposure(&positions, "BTC/USDT");
    assert!((result - 20.0).abs() < 1e-10);
}

#[test]
fn r4_3_calc_symbol_exposure_multi_symbol() {
    let pos1 = make_position("BTC/USDT", PositionSide::Long, 2.0, 100.0, 100.0, 10, None);
    let pos2 = make_position("ETH/USDT", PositionSide::Long, 3.0, 50.0, 50.0, 5, None);
    // BTC margin = 20, ETH margin = 30
    let positions: Vec<&Position> = vec![&pos1, &pos2];
    let result = calc_symbol_exposure(&positions, "BTC/USDT");
    assert!((result - 20.0).abs() < 1e-10);
}

// ── calc_total_exposure ────────────────────────────────────

#[test]
fn r5_1_calc_total_exposure_empty() {
    let positions: Vec<&Position> = vec![];
    let result = calc_total_exposure(&positions);
    assert!((result - 0.0).abs() < 1e-10);
}

#[test]
fn r5_2_calc_total_exposure_multi() {
    let pos1 = make_position("BTC/USDT", PositionSide::Long, 2.0, 100.0, 100.0, 10, None);
    let pos2 = make_position("ETH/USDT", PositionSide::Long, 3.0, 50.0, 50.0, 5, None);
    // BTC margin = 20, ETH margin = 30, total = 50
    let positions: Vec<&Position> = vec![&pos1, &pos2];
    let result = calc_total_exposure(&positions);
    assert!((result - 50.0).abs() < 1e-10);
}
