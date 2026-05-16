use super::common::*;
use crate::bot::semi_automatic_grid::ai::*;
use crate::bot::semi_automatic_grid::ports::CredentialStore;
use uuid::Uuid;

// ── GridAction::as_str ──

#[test]
fn grid_action_as_str_all_variants() {
    assert_eq!(GridAction::RunGrid.as_str(), "run_grid");
    assert_eq!(GridAction::PauseGrid.as_str(), "pause_grid");
    assert_eq!(GridAction::AdjustGrid { upper_price: None, lower_price: None }.as_str(), "adjust_grid");
    assert_eq!(GridAction::AdjustGrid { upper_price: Some(65000.0), lower_price: Some(45000.0) }.as_str(), "adjust_grid");
    assert_eq!(GridAction::ReducePosition.as_str(), "reduce_position");
    assert_eq!(GridAction::Hold.as_str(), "hold");
}

// ── GridAction::from_str ──

#[test]
fn grid_action_from_str_known_actions() {
    assert!(matches!(GridAction::from_str("run_grid", None, None), GridAction::RunGrid));
    assert!(matches!(GridAction::from_str("pause_grid", None, None), GridAction::PauseGrid));
    assert!(matches!(GridAction::from_str("reduce_position", None, None), GridAction::ReducePosition));
    assert!(matches!(GridAction::from_str("hold", None, None), GridAction::Hold));
}

#[test]
fn grid_action_from_str_adjust_grid_with_prices() {
    let action = GridAction::from_str("adjust_grid", Some(65000.0), Some(48000.0));
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
    let action = GridAction::from_str("adjust_grid", None, None);
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
    let action = GridAction::from_str("adjust_grid", Some(65000.0), None);
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
    assert!(matches!(GridAction::from_str("unknown_action", None, None), GridAction::Hold));
    assert!(matches!(GridAction::from_str("", None, None), GridAction::Hold));
    assert!(matches!(GridAction::from_str("RUN_GRID", None, None), GridAction::RunGrid));
    assert!(matches!(GridAction::from_str("pause_grid", None, None), GridAction::PauseGrid));
    assert!(matches!(GridAction::from_str("PauseGrid", None, None), GridAction::Hold));
}

// ── GridDecision::from_json ──

#[test]
fn grid_decision_from_json_run_grid() {
    let json = serde_json::json!({
        "action": "run_grid",
        "reason": "Price is in range"
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
        "action": "pause_grid",
        "reason": "Price exceeded upper bound",
        "upper_price": 62000.0,
        "lower_price": 48000.0
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
        "action": "adjust_grid",
        "reason": "Volatility increased",
        "upper_price": 65000.0,
        "lower_price": 45000.0
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
        "action": "reduce_position",
        "reason": "High drawdown"
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::ReducePosition));
    assert_eq!(decision.reason, "High drawdown");
}

#[test]
fn grid_decision_from_json_hold() {
    let json = serde_json::json!({
        "action": "hold",
        "reason": "No change needed"
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_unknown_action_defaults_hold() {
    let json = serde_json::json!({
        "action": "something_else",
        "reason": "Unknown"
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_missing_action_defaults_hold() {
    let json = serde_json::json!({
        "reason": "No action field"
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
        "action": "run_grid"
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::RunGrid));
    assert_eq!(decision.reason, "No reason provided");
}

#[test]
fn grid_decision_from_json_non_string_action() {
    let json = serde_json::json!({
        "action": 42,
        "reason": "Action is number"
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_non_string_reason() {
    let json = serde_json::json!({
        "action": "hold",
        "reason": 123
    });
    let decision = GridDecision::from_json(&json);
    assert_eq!(decision.reason, "No reason provided");
}

#[test]
fn grid_decision_from_json_upper_price_not_number() {
    let json = serde_json::json!({
        "action": "adjust_grid",
        "reason": "test",
        "upper_price": "not_a_number",
        "lower_price": null
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
        "action": "adjust_grid",
        "reason": "test",
        "upper_price": null,
        "lower_price": null
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
        "action": "adjust_grid",
        "reason": "test",
        "upper_price": -100.0,
        "lower_price": -200.0
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
        "action": "adjust_grid",
        "reason": "test",
        "upper_price": 0.0,
        "lower_price": 0.0
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
        "action": null,
        "reason": "null action"
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_action_is_boolean() {
    let json = serde_json::json!({
        "action": true,
        "reason": "bool action"
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_action_is_object() {
    let json = serde_json::json!({
        "action": { "type": "run_grid" },
        "reason": "object action"
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::Hold));
}

#[test]
fn grid_decision_from_json_reason_is_array() {
    let json = serde_json::json!({
        "action": "hold",
        "reason": ["multiple", "reasons"]
    });
    let decision = GridDecision::from_json(&json);
    assert_eq!(decision.reason, "No reason provided");
}

#[test]
fn grid_decision_from_json_extra_fields_ignored() {
    let json = serde_json::json!({
        "action": "hold",
        "reason": "test",
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
        "action": "adjust_grid",
        "reason": "test",
        "upper_price": 65000,
        "lower_price": 45000
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
        "action": "hold",
        "reason": long_reason
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
        "action": "adjust_grid",
        "reason": "test consistency",
        "upper_price": 70000.0,
        "lower_price": 40000.0
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
        "action": "pause_grid",
        "reason": "test",
        "upper_price": 65000.0,
        "lower_price": 45000.0
    });
    let decision = GridDecision::from_json(&json);
    assert!(matches!(decision.action, GridAction::PauseGrid));
    assert_eq!(decision.upper_price, Some(65000.0));
    assert_eq!(decision.lower_price, Some(45000.0));
}

#[test]
fn grid_decision_from_json_very_small_prices() {
    let json = serde_json::json!({
        "action": "adjust_grid",
        "reason": "test",
        "upper_price": 0.001,
        "lower_price": 0.0001
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
        "action": "adjust_grid",
        "reason": "test",
        "upper_price": 1e10,
        "lower_price": 1e9
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
        "action": "adjust_grid",
        "reason": "test",
        "upper_price": 65000.123456,
        "lower_price": 45000.789012
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
        "action": "adjust_grid",
        "reason": "test",
        "upper_price": 40000.0,
        "lower_price": 60000.0
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
        upper_price: None,
        lower_price: None,
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
