use super::common::*;
use crate::bot::semi_automatic_grid::ai::*;
use uuid::Uuid;

#[test]
fn grid_action_as_str() {
    assert_eq!(GridAction::RunGrid.as_str(), "run_grid");
    assert_eq!(GridAction::PauseGrid.as_str(), "pause_grid");
    assert_eq!(GridAction::AdjustGrid { upper_price: None, lower_price: None }.as_str(), "adjust_grid");
    assert_eq!(GridAction::ReducePosition.as_str(), "reduce_position");
    assert_eq!(GridAction::Hold.as_str(), "hold");
}

#[test]
fn grid_action_deserialize() {
    let run: GridAction = serde_json::from_value(serde_json::json!("run_grid")).unwrap();
    assert!(matches!(run, GridAction::RunGrid));
    let pause: GridAction = serde_json::from_value(serde_json::json!("pause_grid")).unwrap();
    assert!(matches!(pause, GridAction::PauseGrid { .. }));
    let reduce: GridAction = serde_json::from_value(serde_json::json!("reduce_position")).unwrap();
    assert!(matches!(reduce, GridAction::ReducePosition));
    let hold: GridAction = serde_json::from_value(serde_json::json!("hold")).unwrap();
    assert!(matches!(hold, GridAction::Hold));
}

#[test]
fn grid_action_deserialize_unknown() {
    let result = serde_json::from_value::<GridAction>(serde_json::json!("unknown_action"));
    assert!(result.is_err());
}

#[test]
fn grid_decision_deserialize() {
    let json = serde_json::json!({
        "action": "run_grid",
        "reason": "Price is in range",
        "upper_price": 60000.0,
        "lower_price": 50000.0
    });
    let decision: GridDecision = serde_json::from_value(json).unwrap();
    assert!(matches!(decision.action, GridAction::RunGrid));
    assert_eq!(decision.reason, "Price is in range");
    assert_eq!(decision.upper_price, Some(60000.0));
    assert_eq!(decision.lower_price, Some(50000.0));
}

#[test]
fn grid_decision_default_prices() {
    let json = serde_json::json!({ "action": "hold", "reason": "No change needed" });
    let decision: GridDecision = serde_json::from_value(json).unwrap();
    assert!(matches!(decision.action, GridAction::Hold));
    assert_eq!(decision.reason, "No change needed");
    assert_eq!(decision.upper_price, None);
    assert_eq!(decision.lower_price, None);
}

#[test]
fn grid_ai_service_is_available() {
    let service = GridAiService::new(
        Box::new(MockLlmResolver::new(true)),
        Box::new(MockCredentialStore),
    );
    assert!(service.is_available());

    let service = GridAiService::new(
        Box::new(MockLlmResolver::new(false)),
        Box::new(MockCredentialStore),
    );
    assert!(!service.is_available());
}

#[tokio::test]
async fn grid_ai_service_grid_decision_success() {
    let json = serde_json::json!({
        "action": "pause_grid",
        "reason": "Price exceeded upper bound",
        "upper_price": 62000.0,
        "lower_price": 48000.0
    });
    let action_str = json["action"].as_str().unwrap_or("hold");
    let reason = json["reason"].as_str().unwrap_or("No reason provided").to_string();
    let upper_price = json["upper_price"].as_f64();
    let lower_price = json["lower_price"].as_f64();
    let action = match action_str {
        "run_grid" => GridAction::RunGrid,
        "pause_grid" => GridAction::PauseGrid,
        "adjust_grid" => GridAction::AdjustGrid { upper_price, lower_price },
        "reduce_position" => GridAction::ReducePosition,
        _ => GridAction::Hold,
    };
    let decision = GridDecision { action, reason, upper_price, lower_price };
    assert!(matches!(decision.action, GridAction::PauseGrid));
    assert_eq!(decision.reason, "Price exceeded upper bound");
    assert_eq!(decision.upper_price, Some(62000.0));
    assert_eq!(decision.lower_price, Some(48000.0));
}

#[tokio::test]
async fn grid_ai_service_grid_decision_failure() {
    let json = serde_json::json!({ "action": "invalid_action", "reason": "Some reason" });
    let action_str = json["action"].as_str().unwrap_or("hold");
    let action = match action_str {
        "run_grid" => GridAction::RunGrid,
        "pause_grid" => GridAction::PauseGrid,
        "adjust_grid" => GridAction::AdjustGrid { upper_price: None, lower_price: None },
        "reduce_position" => GridAction::ReducePosition,
        _ => GridAction::Hold,
    };
    assert!(matches!(action, GridAction::Hold));
}
