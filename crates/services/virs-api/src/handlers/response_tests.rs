//! Unit tests for handlers/response.rs ApiResponse methods.

use crate::handlers::response::ApiResponse;
use serde_json::json;

// ============================================================
// TC-R1: ApiResponse::ok
// ============================================================

#[test]
fn r1_1_ok_success_true() {
    let resp = ApiResponse::ok(json!({"key": "value"}));
    assert!(resp.success);
}

#[test]
fn r1_2_ok_data_preserved() {
    let data = json!({"key": "value"});
    let resp = ApiResponse::ok(data.clone());
    assert_eq!(resp.data, data);
}

#[test]
fn r1_3_ok_message_none() {
    let resp = ApiResponse::ok(json!({}));
    assert!(resp.message.is_none());
}

// ============================================================
// TC-R2: ApiResponse::err
// ============================================================

#[test]
fn r2_1_err_success_false() {
    let resp = ApiResponse::err("something went wrong");
    assert!(!resp.success);
}

#[test]
fn r2_2_err_message_set() {
    let resp = ApiResponse::err("error message");
    assert_eq!(resp.message, Some("error message".to_string()));
}
