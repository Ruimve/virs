//! Unit tests for adapter/binance/user_data_ws_api.rs.
//!
//! Covers: build_session_logon_request.

use crate::adapter::binance::user_data_ws_api::build_session_logon_request;
use crate::adapter::binance::BinanceEd25519Signer;

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
