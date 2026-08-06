use crate::handlers::ai_credentials::{parse_balance_response, parse_models_response};
use serde_json::json;


#[test]
fn ac1_1_normal_models() {
    let data = json!({
        "data": [
            {"id": "deepseek-chat", "owned_by": "deepseek"},
            {"id": "deepseek-reasoner", "owned_by": "deepseek"}
        ]
    });
    let models = parse_models_response(&data);
    assert_eq!(models.len(), 2);
    assert_eq!(models[0]["id"], "deepseek-chat");
    assert_eq!(models[0]["owned_by"], "deepseek");
    assert_eq!(models[1]["id"], "deepseek-reasoner");
}

#[test]
fn ac1_2_empty_data() {
    let data = json!(null);
    let models = parse_models_response(&data);
    assert!(models.is_empty());
}

#[test]
fn ac1_3_no_data_field() {
    let data = json!({"other": "field"});
    let models = parse_models_response(&data);
    assert!(models.is_empty());
}


#[test]
fn ac2_1_balance_infos() {
    let data = json!({
        "balance_infos": [
            {"total_balance": "100.50", "currency": "USD"}
        ]
    });
    let balances = parse_balance_response(&data);
    assert_eq!(balances.len(), 1);
    assert_eq!(balances[0]["total_balance"], "100.50");
    assert_eq!(balances[0]["currency"], "USD");
}

#[test]
fn ac2_2_data_fallback() {
    let data = json!({
        "data": [
            {"total_balance": "200.00", "currency": "CNY"}
        ]
    });
    let balances = parse_balance_response(&data);
    assert_eq!(balances.len(), 1);
    assert_eq!(balances[0]["total_balance"], "200.00");
    assert_eq!(balances[0]["currency"], "CNY");
}

#[test]
fn ac2_3_no_balance_fields() {
    let data = json!({"other": "field"});
    let balances = parse_balance_response(&data);
    assert!(balances.is_empty());
}
