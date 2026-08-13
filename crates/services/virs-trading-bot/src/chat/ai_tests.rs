use crate::chat::ai::{BotAction, BotDecision};

#[test]
fn a1_1_action_from_str_open_long() {
    assert_eq!(BotAction::from_str("open_long"), BotAction::OpenLong);
}

#[test]
fn a1_2_action_from_str_open_short() {
    assert_eq!(BotAction::from_str("open_short"), BotAction::OpenShort);
}

#[test]
fn a1_3_action_from_str_close() {
    assert_eq!(
        BotAction::from_str("close_position"),
        BotAction::ClosePosition
    );
}

#[test]
fn a1_4_action_from_str_hold() {
    assert_eq!(BotAction::from_str("hold"), BotAction::Hold);
}

#[test]
fn a1_5_action_from_str_unknown() {
    assert_eq!(BotAction::from_str("unknown"), BotAction::Hold);
}

#[test]
fn a2_1_action_as_str_all_variants() {
    assert_eq!(BotAction::OpenLong.as_str(), "open_long");
    assert_eq!(BotAction::OpenShort.as_str(), "open_short");
    assert_eq!(BotAction::ClosePosition.as_str(), "close_position");
    assert_eq!(BotAction::Hold.as_str(), "hold");
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

    let decision = BotDecision::from_json(&json).expect("should parse");
    assert_eq!(decision.action, BotAction::OpenLong);
    assert_eq!(decision.reason, "EMA golden cross");
    assert!((decision.confidence - 0.85).abs() < 1e-10);
    assert_eq!(decision.market_regime.as_deref(), Some("trending_up"));
    assert!(decision.funding_rate_warning.is_none());
    assert!(decision.event_impact.is_none());
    assert!(decision.analysis.is_some());
    assert!(decision.risk_warning.is_some());
}

#[test]
fn a3_2_decision_from_json_missing_fields_returns_error() {
    let json = serde_json::json!({});
    let result = BotDecision::from_json(&json);
    assert!(
        result.is_err(),
        "empty JSON should return error, not defaults"
    );
}

#[test]
fn a3_4_decision_from_json_confidence_clamped() {
    let json = serde_json::json!({
        "decision": {
            "action": "hold",
            "reason": "testing clamp",
            "confidence": 1.5
        }
    });
    let decision = BotDecision::from_json(&json).expect("should parse");
    assert!((decision.confidence - 1.0).abs() < 1e-10);
}
