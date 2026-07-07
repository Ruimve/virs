//! Unit tests for lib.rs utility functions.
//!
//! Covers: parse_f64, parse_str, parse_u32, parse_timestamp_ms,
//! build_display_url, mask_signature, extract_error_message.

use serde_json::json;

use crate::{
    build_display_url, extract_error_message, mask_signature, parse_f64, parse_str,
    parse_timestamp_ms, parse_u32,
};

// ============================================================
// TC-L1: parse_f64
// ============================================================

#[test]
fn l1_1_parse_f64_from_number() {
    let v = json!({"price": 12345.67});
    assert_eq!(parse_f64(&v, "price"), Some(12345.67));
}

#[test]
fn l1_2_parse_f64_from_string() {
    let v = json!({"price": "12345.67"});
    assert_eq!(parse_f64(&v, "price"), Some(12345.67));
}

#[test]
fn l1_3_parse_f64_missing_field() {
    let v = json!({"other": 1});
    assert_eq!(parse_f64(&v, "price"), None);
}

#[test]
fn l1_4_parse_f64_null_field() {
    let v = json!({"price": null});
    assert_eq!(parse_f64(&v, "price"), None);
}

#[test]
fn l1_5_parse_f64_invalid_string() {
    let v = json!({"price": "abc"});
    assert_eq!(parse_f64(&v, "price"), None);
}

#[test]
fn l1_6_parse_f64_from_integer() {
    let v = json!({"price": 42});
    assert_eq!(parse_f64(&v, "price"), Some(42.0));
}

// ============================================================
// TC-L2: parse_str
// ============================================================

#[test]
fn l2_1_parse_str_from_string() {
    let v = json!({"symbol": "BTCUSDT"});
    assert_eq!(parse_str(&v, "symbol"), Some("BTCUSDT".to_string()));
}

#[test]
fn l2_2_parse_str_from_i64() {
    let v = json!({"count": 12345});
    assert_eq!(parse_str(&v, "count"), Some("12345".to_string()));
}

#[test]
fn l2_3_parse_str_from_f64() {
    let v = json!({"price": 2.5});
    let result = parse_str(&v, "price");
    assert!(result.is_some());
    // f64 → string may have float representation differences, so just check it parses back
    let s = result.unwrap();
    assert!((s.parse::<f64>().unwrap() - 2.5).abs() < 0.001);
}

#[test]
fn l2_4_parse_str_missing_field() {
    let v = json!({"other": "x"});
    assert_eq!(parse_str(&v, "symbol"), None);
}

// ============================================================
// TC-L4: parse_u32
// ============================================================

#[test]
fn l4_1_parse_u32_from_u64() {
    let v = json!({"leverage": 10});
    assert_eq!(parse_u32(&v, "leverage"), Some(10));
}

#[test]
fn l4_2_parse_u32_from_string() {
    let v = json!({"leverage": "20"});
    assert_eq!(parse_u32(&v, "leverage"), Some(20));
}

#[test]
fn l4_3_parse_u32_missing_field() {
    let v = json!({"other": 1});
    assert_eq!(parse_u32(&v, "leverage"), None);
}

// ============================================================
// TC-L5: build_display_url
// ============================================================

#[test]
fn l5_1_build_display_url_no_params() {
    let params = std::iter::empty::<(&str, &str)>();
    assert_eq!(build_display_url("/api/v3/ping", params), "/api/v3/ping");
}

#[test]
fn l5_2_build_display_url_with_params() {
    let params = [("symbol", "BTCUSDT"), ("limit", "100")].into_iter();
    let url = build_display_url("/api/v3/depth", params);
    assert_eq!(url, "/api/v3/depth?symbol=BTCUSDT&limit=100");
}

#[test]
fn l5_3_build_display_url_masks_signature() {
    let params = [("symbol", "BTCUSDT"), ("signature", "abcdef123456")].into_iter();
    let url = build_display_url("/api/v3/order", params);
    assert!(url.contains("***MASKED***"));
    assert!(!url.contains("abcdef123456"));
}

#[test]
fn l5_4_build_display_url_empty_params() {
    let params: [(&str, &str); 0] = [];
    assert_eq!(
        build_display_url("/api/v3/ping", params.into_iter()),
        "/api/v3/ping"
    );
}

// ============================================================
// TC-L6: mask_signature
// ============================================================

#[test]
fn l6_1_mask_signature_basic() {
    let body = "symbol=BTCUSDT&signature=abcdef123456";
    let masked = mask_signature(body);
    assert!(masked.contains("signature=***MASKED***"));
    assert!(!masked.contains("abcdef123456"));
}

#[test]
fn l6_2_mask_signature_with_trailing_params() {
    let body = "symbol=BTCUSDT&signature=abcdef&timestamp=123";
    let masked = mask_signature(body);
    assert!(masked.contains("signature=***MASKED***"));
    assert!(masked.contains("timestamp=123"));
    assert!(!masked.contains("abcdef"));
}

#[test]
fn l6_3_mask_signature_no_signature() {
    let body = "symbol=BTCUSDT&timestamp=123";
    let masked = mask_signature(body);
    assert_eq!(masked, body);
}

#[test]
fn l6_4_mask_signature_multiple_signatures() {
    // Only the first signature= occurrence is masked
    let body = "signature=aaa&signature=bbb";
    let masked = mask_signature(body);
    assert!(masked.contains("***MASKED***"));
    assert!(masked.contains("bbb"));
}

// ============================================================
// TC-L7: extract_error_message
// ============================================================

#[test]
fn l7_1_extract_error_msg_with_code() {
    let json = json!({"code": -1121, "msg": "Invalid symbol."});
    let msg = extract_error_message(&json);
    assert_eq!(msg, "[-1121] Invalid symbol.");
}

#[test]
fn l7_2_extract_error_msg_only() {
    let json = json!({"msg": "Some error"});
    let msg = extract_error_message(&json);
    assert_eq!(msg, "Some error");
}

#[test]
fn l7_3_extract_error_bybit_format() {
    let json = json!({"retCode": 10001, "retMsg": "error"});
    let msg = extract_error_message(&json);
    assert_eq!(msg, "[10001] error");
}

#[test]
fn l7_4_extract_error_error_field() {
    let json = json!({"error": "Not found"});
    let msg = extract_error_message(&json);
    assert_eq!(msg, "Not found");
}

#[test]
fn l7_5_extract_error_message_field() {
    let json = json!({"message": "Bad request"});
    let msg = extract_error_message(&json);
    assert_eq!(msg, "Bad request");
}

#[test]
fn l7_6_extract_error_detail_field() {
    let json = json!({"detail": "Validation failed"});
    let msg = extract_error_message(&json);
    assert_eq!(msg, "Validation failed");
}

#[test]
fn l7_7_extract_error_no_matching_field() {
    let json = json!({"foo": "bar"});
    let msg = extract_error_message(&json);
    assert_eq!(msg, r#"{"foo":"bar"}"#);
}

// ============================================================
// TC-L8: parse_timestamp_ms
// ============================================================

#[test]
fn l8_1_parse_timestamp_ms_from_i64() {
    // 2024-04-15 12:00:00 UTC = 1713182400000 ms
    let v = json!({"time": 1713182400000i64});
    let dt = parse_timestamp_ms(&v, "time");
    assert!(dt.is_some());
    let dt = dt.unwrap();
    assert_eq!(dt.timestamp_millis(), 1713182400000);
}

#[test]
fn l8_2_parse_timestamp_ms_from_string() {
    let v = json!({"transactTime": "1713182400000"});
    let dt = parse_timestamp_ms(&v, "transactTime");
    assert!(dt.is_some());
    let dt = dt.unwrap();
    assert_eq!(dt.timestamp_millis(), 1713182400000);
}

#[test]
fn l8_3_parse_timestamp_ms_missing_field() {
    let v = json!({"other": 1});
    assert_eq!(parse_timestamp_ms(&v, "time"), None);
}

#[test]
fn l8_4_parse_timestamp_ms_invalid_string() {
    let v = json!({"time": "not_a_number"});
    assert_eq!(parse_timestamp_ms(&v, "time"), None);
}
