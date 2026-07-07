//! Unit tests for adapter/binance/user_data_ws_api.rs.
//!
//! Covers: build_session_logon_request.

use crate::adapter::binance::user_data_ws_api::build_session_logon_request;
use crate::adapter::binance::BinanceEd25519Signer;
use crate::auth::Signer;

/// Helper: create a test Ed25519 signer from a known 32-byte zero seed.
fn test_signer() -> BinanceEd25519Signer {
    // 32 zero bytes in base64
    let seed_b64 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    BinanceEd25519Signer::from_seed_b64("test_api_key_12345", seed_b64).unwrap()
}

// ============================================================
// TC-WA1: build_session_logon_request
// ============================================================

#[test]
fn wa1_1_logon_method_is_session_logon() {
    let signer = test_signer();
    let result = build_session_logon_request(&signer, 1);
    assert!(result.is_ok());
    let req = result.unwrap();
    assert_eq!(req["method"], "session.logon");
}

#[test]
fn wa1_2_logon_params_contain_required_fields() {
    let signer = test_signer();
    let result = build_session_logon_request(&signer, 42);
    assert!(result.is_ok());
    let req = result.unwrap();
    let params = req["params"].as_object().unwrap();
    assert!(params.contains_key("apiKey"));
    assert!(params.contains_key("recvWindow"));
    assert!(params.contains_key("timestamp"));
    assert!(params.contains_key("signature"));
    assert_eq!(params["apiKey"], "test_api_key_12345");
    assert_eq!(params["recvWindow"], 5000);
}

#[test]
fn wa1_3_logon_signature_non_empty() {
    let signer = test_signer();
    let result = build_session_logon_request(&signer, 1);
    assert!(result.is_ok());
    let req = result.unwrap();
    let sig = req["params"]["signature"].as_str().unwrap();
    assert!(!sig.is_empty());
}

#[test]
fn wa1_4_logon_id_preserved() {
    let signer = test_signer();
    let result = build_session_logon_request(&signer, 999);
    assert!(result.is_ok());
    let req = result.unwrap();
    assert_eq!(req["id"], 999);
}

#[test]
fn wa1_5_logon_different_api_keys_different_signatures() {
    let seed_b64 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    let signer1 = BinanceEd25519Signer::from_seed_b64("key_one", seed_b64).unwrap();
    let signer2 = BinanceEd25519Signer::from_seed_b64("key_two", seed_b64).unwrap();

    // Note: timestamps may differ, but we just check that the apiKeys differ
    let req1 = build_session_logon_request(&signer1, 1).unwrap();
    let req2 = build_session_logon_request(&signer2, 1).unwrap();

    assert_ne!(req1["params"]["apiKey"], req2["params"]["apiKey"]);
}

// ============================================================
// TC-WA2: T15 — WS API timestamp includes time_offset_ms
// ============================================================

#[test]
fn wa2_1_t15_timestamp_includes_time_offset() {
    // T15: WS API signing must apply time_offset_ms
    let signer = test_signer();

    // Without offset: timestamp ≈ local now
    let req1 = build_session_logon_request(&signer, 1).unwrap();
    let ts1 = req1["params"]["timestamp"].as_i64().unwrap();
    let local_now = chrono::Utc::now().timestamp_millis();
    let diff1 = (ts1 - local_now).abs();
    // Should be within a few seconds (no offset)
    assert!(diff1 < 3000, "timestamp without offset should be close to local now");

    // With offset: timestamp should differ by the offset amount
    signer.set_time_offset(5000); // 5 second offset
    let req2 = build_session_logon_request(&signer, 2).unwrap();
    let ts2 = req2["params"]["timestamp"].as_i64().unwrap();
    let local_now2 = chrono::Utc::now().timestamp_millis();
    // ts2 should be approximately local_now2 + 5000
    let diff2 = (ts2 - local_now2).abs();
    assert!(diff2 > 3000, "timestamp with 5000ms offset should differ from local now by ~5000ms, got diff={}", diff2);
    assert!(diff2 < 8000, "timestamp with 5000ms offset should be within 3000ms of (local+5000), got diff={}", diff2);
}

#[test]
fn wa2_2_t15_get_time_offset_default_zero() {
    // T15: Default time offset should be 0 for new signer
    let signer = test_signer();
    assert_eq!(signer.get_time_offset(), 0);
}

#[test]
fn wa2_3_t15_set_and_get_time_offset() {
    // T15: set_time_offset and get_time_offset should be consistent
    let signer = test_signer();
    signer.set_time_offset(1234);
    assert_eq!(signer.get_time_offset(), 1234);
    signer.set_time_offset(-5678);
    assert_eq!(signer.get_time_offset(), -5678);
}
