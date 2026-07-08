//! Unit tests for grid/ai.rs

use crate::grid::ai::{parse_grid_decision, GridAction};

// ── GridAction::from_str ───────────────────────────────────

#[test]
fn g1_1_action_from_str_adjust() {
    let result = GridAction::from_str("adjust_grid", 100.0, 90.0);
    match result {
        GridAction::AdjustGrid {
            upper_price,
            lower_price,
        } => {
            assert!((upper_price - 100.0).abs() < 1e-10);
            assert!((lower_price - 90.0).abs() < 1e-10);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn g1_2_action_from_str_pause() {
    let result = GridAction::from_str("pause_grid", 0.0, 0.0);
    assert_eq!(result, GridAction::PauseGrid);
}

#[test]
fn g1_3_action_from_str_run() {
    let result = GridAction::from_str("run_grid", 0.0, 0.0);
    assert_eq!(result, GridAction::RunGrid);
}

#[test]
fn g1_4_action_from_str_reduce() {
    let result = GridAction::from_str("reduce_position", 0.0, 0.0);
    assert_eq!(result, GridAction::ReducePosition);
}

#[test]
fn g1_5_action_from_str_hold() {
    let result = GridAction::from_str("unknown_action", 0.0, 0.0);
    assert_eq!(result, GridAction::Hold);
}

// ── GridAction::as_str ─────────────────────────────────────

#[test]
fn g1_6_action_as_str_all_variants() {
    assert_eq!(GridAction::Hold.as_str(), "hold");
    assert_eq!(
        GridAction::AdjustGrid {
            upper_price: 0.0,
            lower_price: 0.0
        }
        .as_str(),
        "adjust_grid"
    );
    assert_eq!(GridAction::PauseGrid.as_str(), "pause_grid");
    assert_eq!(GridAction::RunGrid.as_str(), "run_grid");
    assert_eq!(GridAction::ReducePosition.as_str(), "reduce_position");
}

// ── parse_grid_decision ────────────────────────────────────

#[test]
fn g2_1_parse_decision_complete() {
    let json = serde_json::json!({
        "decision": {
            "action": "adjust_grid",
            "reason": "Bollinger band narrowing",
            "confidence": 0.8
        },
        "grid": {
            "upper_price": 100.0,
            "lower_price": 90.0,
            "grid_count": 10,
            "grid_profit_pct": 0.5
        },
        "risk": {
            "leverage": 5,
            "quantity_per_grid": 100.0
        },
        "market": {
            "market_regime": "ranging"
        },
        "analysis": "Market in range",
        "risk_warning": "Low volatility"
    });

    let decision = parse_grid_decision(&json).expect("complete JSON should parse");
    assert_eq!(decision.action, "adjust_grid");
    assert_eq!(decision.reason, "Bollinger band narrowing");
    assert!((decision.confidence - 0.8).abs() < 1e-10);
    assert!((decision.upper_price - 100.0).abs() < 1e-10);
    assert!((decision.lower_price - 90.0).abs() < 1e-10);
    assert_eq!(decision.grid_count, 10);
    assert!((decision.grid_profit_pct - 0.5).abs() < 1e-10);
    assert_eq!(decision.leverage, 5);
    assert!((decision.quantity_per_grid - 100.0).abs() < 1e-10);
    assert_eq!(decision.market_regime, "ranging");
}

#[test]
fn g2_2_parse_decision_defaults() {
    let json = serde_json::json!({});
    // leverage 缺失时应返回错误，不再使用默认值
    let result = parse_grid_decision(&json);
    assert!(result.is_err(), "missing leverage should return error");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("leverage"),
        "error should mention leverage, got: {err}"
    );

    // 验证其他字段缺失时仍有默认值（在有 leverage 的情况下）
    let json = serde_json::json!({
        "risk": {
            "leverage": 5
        }
    });
    let decision = parse_grid_decision(&json).expect("should parse with leverage present");
    assert_eq!(decision.action, "hold"); // default
    assert_eq!(decision.reason, "No reason provided"); // default
    assert!((decision.confidence - 0.0).abs() < 1e-10); // default — 0.0, not 0.5
    assert!((decision.upper_price - 0.0).abs() < 1e-10); // default
    assert_eq!(decision.grid_count, 0); // default — 0, not 10
    assert!((decision.grid_profit_pct - 0.0).abs() < 1e-10); // default — 0.0, not 0.5
    assert_eq!(decision.leverage, 5); // from JSON
    assert!((decision.quantity_per_grid - 0.0).abs() < 1e-10); // default — 0.0, not 10.0
    assert_eq!(decision.market_regime, "unknown"); // default — "unknown", not "ranging"
}
