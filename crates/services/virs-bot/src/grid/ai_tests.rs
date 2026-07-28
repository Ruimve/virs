use crate::grid::ai::{parse_grid_decision, GridAction, GridAiDecision};
use virs_strategy::output::{StrategyAction, ToStrategyOutput};
use uuid::Uuid;

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
    assert!((decision.quantity_per_grid - 100.0).abs() < 1e-10);
    assert_eq!(decision.market_regime, "ranging");
}

#[test]
fn g2_2_parse_decision_missing_fields_returns_error() {
    let json = serde_json::json!({});

    let result = parse_grid_decision(&json);
    assert!(
        result.is_err(),
        "empty JSON should return error, not defaults"
    );
}

#[test]
fn g3_1_to_output_adjust_grid() {
    let decision = GridAiDecision {
        action: "adjust_grid".to_string(),
        reason: "narrowing bands".to_string(),
        confidence: 0.8,
        upper_price: 100.0,
        lower_price: 90.0,
        grid_count: 10,
        grid_profit_pct: 0.5,
        quantity_per_grid: 1.0,
        market_regime: "ranging".to_string(),
        analysis: "range bound".to_string(),
        risk_warning: "low vol".to_string(),
    };
    let raw = serde_json::json!({"decision": {"action": "adjust_grid"}});
    let out = decision.to_output(raw.clone(), Some(Uuid::nil()));
    match out.action {
        StrategyAction::AdjustGrid {
            upper_price,
            lower_price,
            grid_count,
            grid_profit_pct,
            quantity_per_grid,
        } => {
            assert!((upper_price - 100.0).abs() < 1e-10);
            assert!((lower_price - 90.0).abs() < 1e-10);
            assert_eq!(grid_count, 10);
            assert!((grid_profit_pct - 0.5).abs() < 1e-10);
            assert!((quantity_per_grid - 1.0).abs() < 1e-10);
        }
        other => panic!("expected AdjustGrid, got {:?}", other),
    }
    assert!(out.action.is_grid_restructure());
    assert!(!out.is_open_position());
    assert!(!out.is_noop());
    assert_eq!(out.market_regime.as_deref(), Some("ranging"));
    assert_eq!(out.decision_raw, raw);
}

#[test]
fn g3_2_to_output_unknown_market_regime_normalized_to_none() {
    let decision = GridAiDecision {
        action: "hold".to_string(),
        reason: "wait".to_string(),
        confidence: 0.3,
        upper_price: 0.0,
        lower_price: 0.0,
        grid_count: 0,
        grid_profit_pct: 0.0,
        quantity_per_grid: 0.0,
        market_regime: "unknown".to_string(),
        analysis: "none".to_string(),
        risk_warning: "none".to_string(),
    };
    let out = decision.to_output(serde_json::json!({}), None);
    assert_eq!(out.action, StrategyAction::Hold);
    assert!(out.is_noop());
    assert!(out.market_regime.is_none(), "unknown should be None");
}

#[test]
fn g3_3_to_output_pause_run_reduce() {
    for (action_str, expected) in [
        ("pause_grid", StrategyAction::PauseGrid),
        ("run_grid", StrategyAction::RunGrid),
        ("reduce_position", StrategyAction::ReducePosition),
    ] {
        let decision = GridAiDecision {
            action: action_str.to_string(),
            reason: String::new(),
            confidence: 0.5,
            upper_price: 0.0,
            lower_price: 0.0,
            grid_count: 0,
            grid_profit_pct: 0.0,
            quantity_per_grid: 0.0,
            market_regime: "unknown".to_string(),
            analysis: String::new(),
            risk_warning: String::new(),
        };
        let out = decision.to_output(serde_json::json!({}), None);
        assert_eq!(out.action, expected, "failed for {}", action_str);
        assert!(out.action.is_grid_restructure());
    }
}
