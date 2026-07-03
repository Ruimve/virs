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
