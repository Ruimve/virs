use crate::handlers::response::ApiResponse;
use serde_json::json;
use virs_error::{ApiError, VirsError};


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


#[test]
fn r2_1_err_success_false() {
    let err = ApiError::from(VirsError::bad_request("something went wrong"));
    assert!(!err.success);
}

#[test]
fn r2_2_err_message_set() {
    let err = ApiError::from(VirsError::bad_request("error message"));
    assert_eq!(err.message, "error message");
}
