/**
 * 测试 ai::AutoDecision::from_json LLM返回JSON解析
 * - 完整JSON：所有字段正确解析
 * - 缺失字段：使用默认值
 * - confidence 边界：clamp(0.0, 1.0)
 * - "none" 字符串被过滤为 None
 * - 空 JSON：默认 Hold
 * - 嵌套层级错误：默认 Hold
 */
use crate::bot::auto_trade::ai::{AutoAction, AutoDecision};

#[test]
fn full_json_parses_correctly() {
    let json = serde_json::json!({
        "decision": {
            "action": "open_long",
            "reason": "EMA golden cross",
            "confidence": 0.85
        },
        "market": {
            "market_regime": "trending_up",
            "funding_rate_warning": "high funding",
            "event_impact": "none"
        },
        "analysis": "Strong uptrend",
        "risk_warning": "Volatile market"
    });

    let d = AutoDecision::from_json(&json);
    assert_eq!(d.action, AutoAction::OpenLong);
    assert_eq!(d.reason, "EMA golden cross");
    assert!((d.confidence - 0.85).abs() < 0.001);
    assert_eq!(d.market_regime.as_deref(), Some("trending_up"));
    assert_eq!(d.funding_rate_warning.as_deref(), Some("high funding"));
    assert_eq!(d.event_impact, None);
    assert_eq!(d.analysis.as_deref(), Some("Strong uptrend"));
    assert_eq!(d.risk_warning.as_deref(), Some("Volatile market"));
}

#[test]
fn missing_decision_defaults_to_hold() {
    let json = serde_json::json!({});
    let d = AutoDecision::from_json(&json);
    assert_eq!(d.action, AutoAction::Hold);
    assert_eq!(d.reason, "No reason provided");
}

#[test]
fn missing_action_defaults_to_hold() {
    let json = serde_json::json!({
        "decision": { "reason": "test" }
    });
    let d = AutoDecision::from_json(&json);
    assert_eq!(d.action, AutoAction::Hold);
}

#[test]
fn missing_reason_defaults() {
    let json = serde_json::json!({
        "decision": { "action": "hold" }
    });
    let d = AutoDecision::from_json(&json);
    assert_eq!(d.reason, "No reason provided");
}

#[test]
fn confidence_clamped_above_1() {
    let json = serde_json::json!({
        "decision": { "action": "hold", "confidence": 1.5 }
    });
    let d = AutoDecision::from_json(&json);
    assert!((d.confidence - 1.0).abs() < 0.001);
}

#[test]
fn confidence_clamped_below_0() {
    let json = serde_json::json!({
        "decision": { "action": "hold", "confidence": -0.5 }
    });
    let d = AutoDecision::from_json(&json);
    assert!((d.confidence - 0.0).abs() < 0.001);
}

#[test]
fn confidence_default_when_missing() {
    let json = serde_json::json!({
        "decision": { "action": "hold" }
    });
    let d = AutoDecision::from_json(&json);
    assert!((d.confidence - 0.5).abs() < 0.001);
}

#[test]
fn none_string_filtered_in_funding_rate_warning() {
    let json = serde_json::json!({
        "decision": { "action": "hold" },
        "market": { "funding_rate_warning": "none" }
    });
    let d = AutoDecision::from_json(&json);
    assert_eq!(d.funding_rate_warning, None);
}

#[test]
fn none_string_filtered_in_event_impact() {
    let json = serde_json::json!({
        "decision": { "action": "hold" },
        "market": { "event_impact": "None" }
    });
    let d = AutoDecision::from_json(&json);
    assert_eq!(d.event_impact, None);
}

#[test]
fn none_string_filtered_in_risk_warning() {
    let json = serde_json::json!({
        "decision": { "action": "hold" },
        "risk_warning": "NONE"
    });
    let d = AutoDecision::from_json(&json);
    assert_eq!(d.risk_warning, None);
}

#[test]
fn missing_market_defaults_to_none() {
    let json = serde_json::json!({
        "decision": { "action": "hold" }
    });
    let d = AutoDecision::from_json(&json);
    assert_eq!(d.market_regime, None);
    assert_eq!(d.funding_rate_warning, None);
    assert_eq!(d.event_impact, None);
}

#[test]
fn close_position_action() {
    let json = serde_json::json!({
        "decision": { "action": "close_position", "reason": "stop loss", "confidence": 0.9 }
    });
    let d = AutoDecision::from_json(&json);
    assert_eq!(d.action, AutoAction::ClosePosition);
}

#[test]
fn open_short_action() {
    let json = serde_json::json!({
        "decision": { "action": "open_short", "reason": "death cross", "confidence": 0.7 }
    });
    let d = AutoDecision::from_json(&json);
    assert_eq!(d.action, AutoAction::OpenShort);
}
