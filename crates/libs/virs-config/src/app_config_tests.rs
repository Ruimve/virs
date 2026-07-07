//! Unit tests for app_config.rs pure functions and config builders.
//!
//! Covers: parse_env_num and default value constants.

use crate::app_config::*;

// ============================================================
// TC-C3: parse_env_num
// ============================================================

#[test]
fn c3_1_parse_env_num_with_value() {
    let result: Result<u16, _> = parse_env_num(Some("8080".into()), "80");
    assert_eq!(result.unwrap(), 8080);
}

#[test]
fn c3_2_parse_env_num_none_uses_default() {
    let result: Result<u16, _> = parse_env_num(None, "80");
    assert_eq!(result.unwrap(), 80);
}

#[test]
fn c3_3_parse_env_num_invalid_returns_err() {
    let result: Result<u16, _> = parse_env_num(Some("invalid".into()), "80");
    assert!(result.is_err());
}

#[test]
fn c3_4_parse_env_num_empty_string_returns_err() {
    let result: Result<u16, _> = parse_env_num(Some("".into()), "80");
    assert!(result.is_err());
}

#[test]
fn c3_5_parse_env_num_type_safety() {
    let r_u16: u16 = parse_env_num(Some("42".into()), "0").unwrap();
    assert_eq!(r_u16, 42u16);

    let r_u32: u32 = parse_env_num(Some("42".into()), "0").unwrap();
    assert_eq!(r_u32, 42u32);

    let r_u64: u64 = parse_env_num(Some("42".into()), "0").unwrap();
    assert_eq!(r_u64, 42u64);

    let r_i64: i64 = parse_env_num(Some("-5".into()), "0").unwrap();
    assert_eq!(r_i64, -5i64);
}

// ============================================================
// TC-S3: Default value constants verification
// ============================================================

#[test]
fn s3_1_default_constants_values() {
    assert_eq!(DEFAULT_HOST, "0.0.0.0");
    assert_eq!(DEFAULT_PORT, "8080");
    assert_eq!(DEFAULT_JWT_HOURS, "24");
    assert_eq!(DEFAULT_DB_POOL_MIN, "5");
    assert_eq!(DEFAULT_DB_POOL_MAX, "50");
    // NOTE: ADMIN_USERNAME and ADMIN_PASSWORD no longer have defaults.
    // They must be provided via environment variables — see app_config.rs.
}

// ============================================================
// TC-T12: TimeConfig defaults and env loading
// ============================================================

#[test]
fn t12_1_time_config_default_values() {
    // T12: TimeConfig::default() should return the expected default values
    let tc = TimeConfig::default();
    assert_eq!(tc.max_position_duration_secs, 172800, "48h = 172800s");
    assert_eq!(tc.pending_order_timeout_secs, 60);
    assert_eq!(tc.price_poll_interval_secs, 5);
    assert_eq!(tc.close_order_timeout_secs, 15);
    assert_eq!(tc.http_timeout_secs, 30);
    assert_eq!(tc.llm_timeout_secs, 120);
}

#[test]
fn t12_2_time_config_default_constants() {
    // T12: Default constant values match expected
    assert_eq!(DEFAULT_MAX_POSITION_DURATION_SECS, "172800");
    assert_eq!(DEFAULT_PENDING_ORDER_TIMEOUT_SECS, "60");
    assert_eq!(DEFAULT_PRICE_POLL_INTERVAL_SECS, "5");
    assert_eq!(DEFAULT_CLOSE_ORDER_TIMEOUT_SECS, "15");
    assert_eq!(DEFAULT_HTTP_TIMEOUT_SECS, "30");
    assert_eq!(DEFAULT_LLM_TIMEOUT_SECS, "120");
}

#[test]
fn t12_3_time_config_serde_roundtrip() {
    // T12: TimeConfig should survive serde round-trip
    let tc = TimeConfig {
        max_position_duration_secs: 3600,
        pending_order_timeout_secs: 30,
        price_poll_interval_secs: 10,
        close_order_timeout_secs: 20,
        http_timeout_secs: 60,
        llm_timeout_secs: 240,
    };
    let json = serde_json::to_string(&tc).unwrap();
    let de: TimeConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de, tc);
}

#[test]
fn t12_4_time_config_clone_and_eq() {
    // T12: TimeConfig should support Clone + PartialEq
    let tc1 = TimeConfig::default();
    let tc2 = tc1.clone();
    assert_eq!(tc1, tc2);
}

#[test]
fn t12_5_time_config_max_position_duration_is_48h() {
    // T12: Critical trading parameter — max position duration must be 48h
    let tc = TimeConfig::default();
    let hours = tc.max_position_duration_secs / 3600;
    assert_eq!(hours, 48, "MAX_POSITION_DURATION must be 48 hours");
}
