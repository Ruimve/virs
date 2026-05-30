use super::common::*;
use crate::bot::semi_automatic_grid::ai::*;
use crate::bot::semi_automatic_grid::ports::CredentialStore;
use uuid::Uuid;

// ── GridAction::as_str ──

#[test]
fn grid_action_as_str_all_variants() {
    assert_eq!(GridAction::RunGrid.as_str(), "resume_grid");
    assert_eq!(GridAction::PauseGrid.as_str(), "pause_grid");
    assert_eq!(GridAction::AdjustGrid { upper_price: None, lower_price: None }.as_str(), "adjust_grid");
    assert_eq!(GridAction::AdjustGrid { upper_price: Some(65000.0), lower_price: Some(45000.0) }.as_str(), "adjust_grid");
    assert_eq!(GridAction::ReducePosition.as_str(), "reduce_position");
    assert_eq!(GridAction::CancelOrder { level: 3, side: "buy".to_string() }.as_str(), "cancel_order");
    assert_eq!(GridAction::Hold.as_str(), "hold");
}

// ── GridAction::from_str ──

#[test]
fn grid_action_from_str_known_actions() {
    assert!(matches!(GridAction::from_str("resume_grid", None, None, None, None), GridAction::RunGrid));
    assert!(matches!(GridAction::from_str("pause_grid", None, None, None, None), GridAction::PauseGrid));
    assert!(matches!(GridAction::from_str("reduce_position", None, None, None, None), GridAction::ReducePosition));
    assert!(matches!(GridAction::from_str("hold", None, None, None, None), GridAction::Hold));
}

#[test]
fn grid_action_from_str_adjust_grid_with_prices() {
    let action = GridAction::from_str("adjust_grid", Some(65000.0), Some(48000.0), None, None);
    match action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, Some(65000.0));
            assert_eq!(lower_price, Some(48000.0));
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_action_from_str_adjust_grid_with_none_prices() {
    let action = GridAction::from_str("adjust_grid", None, None, None, None);
    match action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, None);
            assert_eq!(lower_price, None);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_action_from_str_adjust_grid_with_partial_prices() {
    let action = GridAction::from_str("adjust_grid", Some(65000.0), None, None, None);
    match action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, Some(65000.0));
            assert_eq!(lower_price, None);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_action_from_str_unknown_falls_to_hold() {
    assert!(matches!(GridAction::from_str("unknown_action", None, None, None, None), GridAction::Hold));
    assert!(matches!(GridAction::from_str("", None, None, None, None), GridAction::Hold));
    assert!(matches!(GridAction::from_str("RESUME_GRID", None, None, None, None), GridAction::RunGrid));
    assert!(matches!(GridAction::from_str("pause_grid", None, None, None, None), GridAction::PauseGrid));
    assert!(matches!(GridAction::from_str("PauseGrid", None, None, None, None), GridAction::Hold));
}

// ── GridDecision::from_json ──

#[test]
fn grid_decision_from_json_decision_action_field() {
    let json = serde_json::json!({
        "decision": { "action": "pause_grid", "reason": "Price exceeded upper bound", "confidence": 0.8 }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::PauseGrid));
    assert_eq!(decision.reason, "Price exceeded upper bound");
}

#[test]
fn grid_decision_from_json_resume_grid_action() {
    let json = serde_json::json!({
        "decision": { "action": "resume_grid", "reason": "Market stabilized", "confidence": 0.7 }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::RunGrid));
}

#[test]
fn grid_decision_from_json_run_grid() {
    let json = serde_json::json!({
        "decision": { "action": "resume_grid", "reason": "Price is in range", "confidence": 0.6 }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::RunGrid));
    assert_eq!(decision.reason, "Price is in range");
    assert_eq!(decision.upper_price, None);
    assert_eq!(decision.lower_price, None);
}

#[test]
fn grid_decision_from_json_pause_grid() {
    let json = serde_json::json!({
        "decision": { "action": "pause_grid", "reason": "Price exceeded upper bound", "confidence": 0.9 },
        "grid": { "upper_price": 62000.0, "lower_price": 48000.0 }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::PauseGrid));
    assert_eq!(decision.reason, "Price exceeded upper bound");
    assert_eq!(decision.upper_price, Some(62000.0));
    assert_eq!(decision.lower_price, Some(48000.0));
}

#[test]
fn grid_decision_from_json_adjust_grid() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "Volatility increased", "confidence": 0.75 },
        "grid": { "upper_price": 65000.0, "lower_price": 45000.0 }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, Some(65000.0));
            assert_eq!(lower_price, Some(45000.0));
        }
        _ => panic!("Expected AdjustGrid"),
    }
    assert_eq!(decision.reason, "Volatility increased");
}

#[test]
fn grid_decision_from_json_reduce_position() {
    let json = serde_json::json!({
        "decision": { "action": "reduce_position", "reason": "High drawdown", "confidence": 0.7 }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::ReducePosition));
    assert_eq!(decision.reason, "High drawdown");
}

#[test]
fn grid_decision_from_json_hold() {
    let json = serde_json::json!({
        "decision": { "action": "hold", "reason": "No change needed", "confidence": 0.5 }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_unknown_action_defaults_hold() {
    let json = serde_json::json!({
        "decision": { "action": "something_else", "reason": "Unknown", "confidence": 0.3 }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_missing_action_defaults_hold() {
    let json = serde_json::json!({
        "decision": { "reason": "No action field" }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
    assert_eq!(decision.reason, "No action field");
}

#[test]
fn grid_decision_from_json_empty_object() {
    let json = serde_json::json!({});
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
    assert_eq!(decision.reason, "No reason provided");
}

#[test]
fn grid_decision_from_json_missing_reason_defaults() {
    let json = serde_json::json!({
        "decision": { "action": "resume_grid" }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::RunGrid));
    assert_eq!(decision.reason, "No reason provided");
}

#[test]
fn grid_decision_from_json_non_string_action() {
    let json = serde_json::json!({
        "decision": { "action": 42, "reason": "Action is number" }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_non_string_reason() {
    let json = serde_json::json!({
        "decision": { "action": "hold", "reason": 123 }
    });
    let decision = GridDecision::from_json(&json);
    assert_eq!(decision.reason, "No reason provided");
}

#[test]
fn grid_decision_from_json_upper_price_not_number() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test" },
        "grid": { "upper_price": "not_a_number", "lower_price": null }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, None);
            assert_eq!(lower_price, None);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_decision_from_json_null_prices() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test" },
        "grid": { "upper_price": null, "lower_price": null }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, None);
            assert_eq!(lower_price, None);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_decision_from_json_negative_prices() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test" },
        "grid": { "upper_price": -100.0, "lower_price": -200.0 }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, None);
            assert_eq!(lower_price, None);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_decision_from_json_zero_prices() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test" },
        "grid": { "upper_price": 0.0, "lower_price": 0.0 }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, None);
            assert_eq!(lower_price, None);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_decision_from_json_action_is_null() {
    let json = serde_json::json!({
        "decision": { "action": null, "reason": "null action" }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_action_is_boolean() {
    let json = serde_json::json!({
        "decision": { "action": true, "reason": "bool action" }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_action_is_object() {
    let json = serde_json::json!({
        "decision": { "action": { "type": "resume_grid" }, "reason": "object action" }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_reason_is_array() {
    let json = serde_json::json!({
        "decision": { "action": "hold", "reason": ["multiple", "reasons"] }
    });
    let decision = GridDecision::from_json(&json);
    assert_eq!(decision.reason, "No reason provided");
}

#[test]
fn grid_decision_from_json_extra_fields_ignored() {
    let json = serde_json::json!({
        "decision": { "action": "hold", "reason": "test" },
        "extra_field": "ignored",
        "another": 123
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
    assert_eq!(decision.reason, "test");
}

#[test]
fn grid_decision_from_json_integer_prices() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test" },
        "grid": { "upper_price": 65000, "lower_price": 45000 }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, Some(65000.0));
            assert_eq!(lower_price, Some(45000.0));
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_decision_from_json_very_long_reason() {
    let long_reason = "a".repeat(10000);
    let json = serde_json::json!({
        "decision": { "action": "hold", "reason": long_reason }
    });
    let decision = GridDecision::from_json(&json);
    assert_eq!(decision.reason.len(), 10000);
}

// ── GridAiService ──

#[test]
fn grid_ai_service_is_available_true() {
    let service = GridAiService::new(
        Box::new(MockLlmResolver::new(true)),
        Box::new(MockCredentialStore::new()),
    );
    assert!(service.is_available());
}

#[test]
fn grid_ai_service_is_available_false() {
    let service = GridAiService::new(
        Box::new(MockLlmResolver::new(false)),
        Box::new(MockCredentialStore::new()),
    );
    assert!(!service.is_available());
}

#[tokio::test]
async fn grid_ai_service_grid_decision_unavailable_returns_none() {
    let service = GridAiService::new(
        Box::new(MockLlmResolver::new(false)),
        Box::new(MockCredentialStore::new()),
    );
    let user_id = Uuid::new_v4();
    let result = service.grid_decision(&user_id, "system", "user").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn grid_ai_service_call_llm_unavailable_fails() {
    let service = GridAiService::new(
        Box::new(MockLlmResolver::new(false)),
        Box::new(MockCredentialStore::new()),
    );
    let user_id = Uuid::new_v4();
    let result = service.call_llm(&user_id, "system", "user").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn grid_ai_service_call_llm_available_but_no_endpoint_fails() {
    let service = GridAiService::new(
        Box::new(MockLlmResolver::new(true)),
        Box::new(MockCredentialStore::new()),
    );
    let user_id = Uuid::new_v4();
    let result = service.call_llm(&user_id, "system", "user").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn grid_ai_service_with_credentials_still_fails_on_bad_endpoint() {
    let service = GridAiService::new(
        Box::new(MockLlmResolver::new(true)),
        Box::new(MockCredentialStore::new().with_creds(vec![("openrouter".to_string(), "test-key".to_string())])),
    );
    let user_id = Uuid::new_v4();
    let result = service.call_llm(&user_id, "system", "user").await;
    assert!(result.is_err());
}

// ── GridDecision 字段一致性 ──

#[test]
fn grid_decision_from_json_adjust_grid_prices_match_decision_fields() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test consistency" },
        "grid": { "upper_price": 70000.0, "lower_price": 40000.0 }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, decision.upper_price);
            assert_eq!(lower_price, decision.lower_price);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_decision_from_json_non_adjust_action_still_extracts_prices() {
    let json = serde_json::json!({
        "decision": { "action": "pause_grid", "reason": "test" },
        "grid": { "upper_price": 65000.0, "lower_price": 45000.0 }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::PauseGrid));
    assert_eq!(decision.upper_price, Some(65000.0));
    assert_eq!(decision.lower_price, Some(45000.0));
}

#[test]
fn grid_decision_from_json_very_small_prices() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test" },
        "grid": { "upper_price": 0.001, "lower_price": 0.0001 }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, Some(0.001));
            assert_eq!(lower_price, Some(0.0001));
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_decision_from_json_very_large_prices() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test" },
        "grid": { "upper_price": 1e10, "lower_price": 1e9 }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert!(upper_price.unwrap() > 1e9);
            assert!(lower_price.unwrap() > 1e8);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_decision_from_json_float_prices() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test" },
        "grid": { "upper_price": 65000.123456, "lower_price": 45000.789012 }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert!((upper_price.unwrap() - 65000.123456).abs() < 0.001);
            assert!((lower_price.unwrap() - 45000.789012).abs() < 0.001);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_decision_from_json_inverted_prices() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test" },
        "grid": { "upper_price": 40000.0, "lower_price": 60000.0 }
    });
    let decision = GridDecision::from_json(&json);
    match decision.action {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, None);
            assert_eq!(lower_price, None);
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

// ── GridAction Debug/Clone 语义 ──

#[test]
fn grid_action_clone() {
    let action = GridAction::AdjustGrid { upper_price: Some(65000.0), lower_price: Some(45000.0) };
    let cloned = action.clone();
    match cloned {
        GridAction::AdjustGrid { upper_price, lower_price } => {
            assert_eq!(upper_price, Some(65000.0));
            assert_eq!(lower_price, Some(45000.0));
        }
        _ => panic!("Expected AdjustGrid"),
    }
}

#[test]
fn grid_decision_clone() {
    let decision = GridDecision {
        action: GridAction::ReducePosition,
        reason: "test clone".to_string(),
        confidence: 0.8,
        upper_price: None,
        lower_price: None,
        cancel_level: None,
        cancel_side: None,
        grid_count: None,
        grid_profit_pct: None,
        quantity_per_grid: None,
        leverage: None,
        market_regime: None,
        analysis: None,
        funding_rate_warning: None,
        event_impact: None,
        risk_warning: None,
    };
    let cloned = decision.clone();
    assert!(matches!(cloned.action, GridAction::ReducePosition));
    assert_eq!(cloned.reason, "test clone");
}

// ── GridAiService credential 加载失败 ──

#[tokio::test]
async fn grid_ai_service_call_llm_credential_load_fails() {
    let service = GridAiService::new(
        Box::new(MockLlmResolver::new(true)),
        Box::new(MockFailingCredentialStore::new()),
    );
    let user_id = Uuid::new_v4();
    let result = service.call_llm(&user_id, "system", "user").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn grid_ai_service_grid_decision_credential_fails_returns_none() {
    let service = GridAiService::new(
        Box::new(MockLlmResolver::new(true)),
        Box::new(MockFailingCredentialStore::new()),
    );
    let user_id = Uuid::new_v4();
    let result = service.grid_decision(&user_id, "system", "user").await;
    assert!(result.is_none());
}

pub struct MockFailingCredentialStore;

impl MockFailingCredentialStore {
    pub fn new() -> Self { Self }
}

#[async_trait::async_trait]
impl CredentialStore for MockFailingCredentialStore {
    async fn load_credentials(&self, _user_id: Uuid) -> anyhow::Result<Vec<(String, String)>> {
        anyhow::bail!("credential load failed")
    }
}

// ── GridDecision 新字段解析 ──

#[test]
fn grid_decision_from_json_full_params() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "Volatility increased", "confidence": 0.8 },
        "grid": { "upper_price": 65000.0, "lower_price": 45000.0, "grid_count": 10, "grid_profit_pct": 0.5 },
        "risk": { "quantity_per_grid": 20.0, "leverage": 3 },
        "market": { "market_regime": "ranging" },
        "analysis": "Market is ranging with moderate volatility"
    });
    let decision = GridDecision::from_json(&json);
    assert_eq!(decision.grid_count, Some(10));
    assert_eq!(decision.grid_profit_pct, Some(0.5));
    assert_eq!(decision.quantity_per_grid, Some(20.0));
    assert_eq!(decision.leverage, Some(3));
    assert_eq!(decision.market_regime, Some("ranging".to_string()));
    assert_eq!(decision.analysis, Some("Market is ranging with moderate volatility".to_string()));
}

#[test]
fn grid_decision_from_json_missing_optional_params() {
    let json = serde_json::json!({
        "decision": { "action": "hold", "reason": "No change needed" }
    });
    let decision = GridDecision::from_json(&json);
    assert_eq!(decision.grid_count, None);
    assert_eq!(decision.grid_profit_pct, None);
    assert_eq!(decision.quantity_per_grid, None);
    assert_eq!(decision.leverage, None);
    assert_eq!(decision.market_regime, None);
    assert_eq!(decision.analysis, None);
}

#[test]
fn grid_decision_from_json_partial_params() {
    let json = serde_json::json!({
        "decision": { "action": "adjust_grid", "reason": "test" },
        "grid": { "upper_price": 70000.0, "lower_price": 50000.0, "grid_count": 8 },
        "risk": { "leverage": 5 }
    });
    let decision = GridDecision::from_json(&json);
    assert_eq!(decision.grid_count, Some(8));
    assert_eq!(decision.grid_profit_pct, None);
    assert_eq!(decision.quantity_per_grid, None);
    assert_eq!(decision.leverage, Some(5));
}

#[test]
fn grid_decision_from_json_as_str_matches_prompt() {
    assert_eq!(GridAction::RunGrid.as_str(), "resume_grid");
    assert_eq!(GridAction::PauseGrid.as_str(), "pause_grid");
    assert_eq!(GridAction::AdjustGrid { upper_price: None, lower_price: None }.as_str(), "adjust_grid");
    assert_eq!(GridAction::ReducePosition.as_str(), "reduce_position");
    assert_eq!(GridAction::CancelOrder { level: 1, side: "buy".to_string() }.as_str(), "cancel_order");
    assert_eq!(GridAction::Hold.as_str(), "hold");
}

#[test]
fn grid_decision_from_json_nested_groups_independent() {
    let json = serde_json::json!({
        "decision": { "action": "cancel_order", "reason": "test" },
        "cancel": { "level": 3, "side": "buy" },
        "grid": { "upper_price": 70000.0, "lower_price": 50000.0 },
        "risk": { "leverage": 5, "quantity_per_grid": 100.0 },
        "market": { "market_regime": "volatile", "funding_rate_warning": "high rate", "event_impact": "FOMC" },
        "analysis": "Market analysis",
        "risk_warning": "High volatility"
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::CancelOrder { .. }));
    assert_eq!(decision.cancel_level, Some(3));
    assert_eq!(decision.cancel_side, Some("buy".to_string()));
    assert_eq!(decision.upper_price, Some(70000.0));
    assert_eq!(decision.lower_price, Some(50000.0));
    assert_eq!(decision.leverage, Some(5));
    assert_eq!(decision.quantity_per_grid, Some(100.0));
    assert_eq!(decision.market_regime, Some("volatile".to_string()));
    assert_eq!(decision.funding_rate_warning, Some("high rate".to_string()));
    assert_eq!(decision.event_impact, Some("FOMC".to_string()));
    assert_eq!(decision.analysis, Some("Market analysis".to_string()));
    assert_eq!(decision.risk_warning, Some("High volatility".to_string()));
}

#[test]
fn grid_decision_from_json_missing_groups_defaults() {
    let json = serde_json::json!({
        "decision": { "action": "hold", "reason": "no groups" }
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
    assert_eq!(decision.upper_price, None);
    assert_eq!(decision.lower_price, None);
    assert_eq!(decision.grid_count, None);
    assert_eq!(decision.grid_profit_pct, None);
    assert_eq!(decision.leverage, None);
    assert_eq!(decision.quantity_per_grid, None);
    assert_eq!(decision.cancel_level, None);
    assert_eq!(decision.cancel_side, None);
    assert_eq!(decision.market_regime, None);
    assert_eq!(decision.funding_rate_warning, None);
    assert_eq!(decision.event_impact, None);
    assert_eq!(decision.analysis, None);
    assert_eq!(decision.risk_warning, None);
}
