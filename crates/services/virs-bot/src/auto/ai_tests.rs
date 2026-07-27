use crate::auto::ai::{AutoAction, AutoDecision};
use virs_strategy::output::{StrategyAction, ToStrategyOutput};
use uuid::Uuid;

#[test]
fn a1_1_action_from_str_open_long() {
    assert_eq!(AutoAction::from_str("open_long"), AutoAction::OpenLong);
}

#[test]
fn a1_2_action_from_str_open_short() {
    assert_eq!(AutoAction::from_str("open_short"), AutoAction::OpenShort);
}

#[test]
fn a1_3_action_from_str_close() {
    assert_eq!(
        AutoAction::from_str("close_position"),
        AutoAction::ClosePosition
    );
}

#[test]
fn a1_4_action_from_str_hold() {
    assert_eq!(AutoAction::from_str("hold"), AutoAction::Hold);
}

#[test]
fn a1_5_action_from_str_unknown() {
    assert_eq!(AutoAction::from_str("unknown"), AutoAction::Hold);
}

#[test]
fn a2_1_action_as_str_all_variants() {
    assert_eq!(AutoAction::OpenLong.as_str(), "open_long");
    assert_eq!(AutoAction::OpenShort.as_str(), "open_short");
    assert_eq!(AutoAction::ClosePosition.as_str(), "close_position");
    assert_eq!(AutoAction::Hold.as_str(), "hold");
}

#[test]
fn a3_1_decision_from_json_complete() {
    let json = serde_json::json!({
        "decision": {
            "action": "open_long",
            "reason": "EMA golden cross",
            "confidence": 0.85
        },
        "market": {
            "market_regime": "trending_up",
            "funding_rate_warning": "none",
            "event_impact": "none"
        },
        "analysis": "Multi-timeframe analysis confirms uptrend",
        "risk_warning": "Watch for RSI divergence"
    });

    let decision = AutoDecision::from_json(&json);
    assert_eq!(decision.action, AutoAction::OpenLong);
    assert_eq!(decision.reason, "EMA golden cross");
    assert!((decision.confidence - 0.85).abs() < 1e-10);
    assert_eq!(decision.market_regime.as_deref(), Some("trending_up"));
    assert!(decision.funding_rate_warning.is_none());
    assert!(decision.event_impact.is_none());
    assert!(decision.analysis.is_some());
    assert!(decision.risk_warning.is_some());
}

#[test]
fn a3_2_decision_from_json_missing_fields() {
    let json = serde_json::json!({});
    let decision = AutoDecision::from_json(&json);
    assert_eq!(decision.action, AutoAction::Hold);
    assert_eq!(decision.reason, "No reason provided");
    assert!((decision.confidence - 0.0).abs() < 1e-10);
    assert!(decision.market_regime.is_none());
}

#[test]
fn a3_4_decision_from_json_confidence_clamped() {
    let json = serde_json::json!({
        "decision": {
            "action": "hold",
            "confidence": 1.5
        }
    });
    let decision = AutoDecision::from_json(&json);
    assert!((decision.confidence - 1.0).abs() < 1e-10);
}

#[test]
fn a4_1_to_output_open_long() {
    let decision = AutoDecision {
        action: AutoAction::OpenLong,
        reason: "bullish".to_string(),
        confidence: 0.8,
        market_regime: Some("trending_up".to_string()),
        funding_rate_warning: None,
        event_impact: None,
        analysis: Some("ema cross".to_string()),
        risk_warning: Some("watch rsi".to_string()),
    };
    let raw = serde_json::json!({"decision": {"action": "open_long"}});
    let out = decision.to_output(raw.clone(), Some(Uuid::nil()));
    assert_eq!(out.action, StrategyAction::OpenLong);
    assert!(out.is_open_position());
    assert!(!out.is_noop());
    assert_eq!(out.reason, "bullish");
    assert!((out.confidence - 0.8).abs() < 1e-10);
    assert_eq!(out.market_regime.as_deref(), Some("trending_up"));
    assert_eq!(out.analysis.as_deref(), Some("ema cross"));
    assert_eq!(out.risk_warning.as_deref(), Some("watch rsi"));
    assert_eq!(out.decision_raw, raw);
    assert!(out.bot_id.is_some());
}

#[test]
fn a4_2_to_output_hold_no_op() {
    let decision = AutoDecision {
        action: AutoAction::Hold,
        reason: "wait".to_string(),
        confidence: 0.3,
        market_regime: None,
        funding_rate_warning: None,
        event_impact: None,
        analysis: None,
        risk_warning: None,
    };
    let out = decision.to_output(serde_json::json!({}), None);
    assert_eq!(out.action, StrategyAction::Hold);
    assert!(out.is_noop());
    assert!(!out.is_open_position());
    assert!(out.bot_id.is_none());
}

#[test]
fn a4_3_to_output_close_position() {
    let decision = AutoDecision {
        action: AutoAction::ClosePosition,
        reason: "take profit".to_string(),
        confidence: 0.9,
        market_regime: None,
        funding_rate_warning: None,
        event_impact: None,
        analysis: None,
        risk_warning: None,
    };
    let out = decision.to_output(serde_json::json!({}), None);
    assert_eq!(out.action, StrategyAction::ClosePosition);
    assert!(!out.is_open_position());
    assert!(!out.is_noop());
}
