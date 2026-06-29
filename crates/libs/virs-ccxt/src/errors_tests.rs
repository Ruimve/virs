//! Unit tests for errors.rs.
//!
//! Covers: ExchangeError::exchange, ExchangeError::no_data, is_retryable.

use crate::errors::ExchangeError;

// ============================================================
// TC-E1: ExchangeError::exchange
// ============================================================

#[test]
fn e1_1_exchange_error_construction() {
    let err = ExchangeError::exchange("-1121", "Invalid symbol.");
    match err {
        ExchangeError::ExchangeError { code, message } => {
            assert_eq!(code, "-1121");
            assert_eq!(message, "Invalid symbol.");
        }
        _ => panic!("Expected ExchangeError variant"),
    }
}

// ============================================================
// TC-E2: ExchangeError::no_data
// ============================================================

#[test]
fn e2_1_no_data_construction() {
    let err = ExchangeError::no_data("No ticker found for BTC/USDT".to_string());
    match err {
        ExchangeError::NoData(msg) => {
            assert_eq!(msg, "No ticker found for BTC/USDT");
        }
        _ => panic!("Expected NoData variant"),
    }
}

// ============================================================
// TC-E3: is_retryable
// ============================================================

#[test]
fn e3_1_network_is_retryable() {
    assert!(ExchangeError::Network("timeout".into()).is_retryable());
}

#[test]
fn e3_2_rate_limited_is_retryable() {
    assert!(ExchangeError::RateLimited("too many requests".into()).is_retryable());
}

#[test]
fn e3_3_internal_is_retryable() {
    assert!(ExchangeError::Internal("unexpected error".into()).is_retryable());
}

#[test]
fn e3_4_http_429_is_retryable() {
    assert!(ExchangeError::Http { status: 429, body: "rate limit".into() }.is_retryable());
}

#[test]
fn e3_5_http_500_is_retryable() {
    assert!(ExchangeError::Http { status: 500, body: "internal server error".into() }.is_retryable());
}

#[test]
fn e3_6_http_502_is_retryable() {
    assert!(ExchangeError::Http { status: 502, body: "bad gateway".into() }.is_retryable());
}

#[test]
fn e3_7_http_503_is_retryable() {
    assert!(ExchangeError::Http { status: 503, body: "service unavailable".into() }.is_retryable());
}

#[test]
fn e3_8_http_504_is_retryable() {
    assert!(ExchangeError::Http { status: 504, body: "gateway timeout".into() }.is_retryable());
}

#[test]
fn e3_9_http_400_not_retryable() {
    assert!(!ExchangeError::Http { status: 400, body: "bad request".into() }.is_retryable());
}

#[test]
fn e3_10_http_401_not_retryable() {
    assert!(!ExchangeError::Http { status: 401, body: "unauthorized".into() }.is_retryable());
}

#[test]
fn e3_11_authentication_not_retryable() {
    assert!(!ExchangeError::Authentication("invalid key".into()).is_retryable());
}

#[test]
fn e3_12_invalid_request_not_retryable() {
    assert!(!ExchangeError::InvalidRequest("bad params".into()).is_retryable());
}

#[test]
fn e3_13_order_not_found_not_retryable() {
    assert!(!ExchangeError::OrderNotFound("order #123".into()).is_retryable());
}

#[test]
fn e3_14_not_supported_not_retryable() {
    assert!(!ExchangeError::NotSupported("OKX not implemented".into()).is_retryable());
}
