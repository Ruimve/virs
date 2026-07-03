//! Unit tests for app_config.rs pure functions and config builders.
//!
//! Covers: parse_bool_str, parse_paper_value, parse_env_num,
//! build_redis_config, build_telegram_config, build_email_config,
//! and default value constants.

use crate::app_config::*;

// ============================================================
// TC-C1: parse_paper_value
// ============================================================

#[test]
fn c1_1_parse_paper_value_true() {
    assert_eq!(parse_paper_value(Some("true".into())), Some(true));
}

#[test]
fn c1_2_parse_paper_value_one() {
    assert_eq!(parse_paper_value(Some("1".into())), Some(true));
}

#[test]
fn c1_3_parse_paper_value_false() {
    assert_eq!(parse_paper_value(Some("false".into())), Some(false));
}

#[test]
fn c1_4_parse_paper_value_zero() {
    assert_eq!(parse_paper_value(Some("0".into())), Some(false));
}

#[test]
fn c1_5_parse_paper_value_other() {
    assert_eq!(parse_paper_value(Some("anything_else".into())), Some(false));
}

#[test]
fn c1_6_parse_paper_value_none_defaults_to_true() {
    assert_eq!(parse_paper_value(None), Some(true));
}

// ============================================================
// TC-C2: parse_bool_str
// ============================================================

#[test]
fn c2_1_parse_bool_str_true() {
    assert!(parse_bool_str("true"));
}

#[test]
fn c2_2_parse_bool_str_one() {
    assert!(parse_bool_str("1"));
}

#[test]
fn c2_3_parse_bool_str_false() {
    assert!(!parse_bool_str("false"));
}

#[test]
fn c2_4_parse_bool_str_zero() {
    assert!(!parse_bool_str("0"));
}

#[test]
fn c2_5_parse_bool_str_yes_is_false() {
    assert!(!parse_bool_str("yes"));
}

#[test]
fn c2_6_parse_bool_str_empty() {
    assert!(!parse_bool_str(""));
}

#[test]
fn c2_7_parse_bool_str_case_sensitive() {
    assert!(!parse_bool_str("TRUE"));
}

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
// TC-C4: build_redis_config
// ============================================================

#[test]
fn c4_1_build_redis_config_with_password() {
    let config = build_redis_config(
        Some("redis://localhost:6379".into()),
        Some("secret".into()),
    );
    assert!(config.is_some());
    let redis = config.unwrap();
    assert_eq!(redis.url, "redis://localhost:6379");
    assert_eq!(redis.password, Some("secret".into()));
}

#[test]
fn c4_2_build_redis_config_without_password() {
    let config = build_redis_config(
        Some("redis://localhost:6379".into()),
        None,
    );
    assert!(config.is_some());
    let redis = config.unwrap();
    assert_eq!(redis.url, "redis://localhost:6379");
    assert_eq!(redis.password, None);
}

#[test]
fn c4_3_build_redis_config_no_url() {
    let config = build_redis_config(None, Some("secret".into()));
    assert_eq!(config, None);
}

#[test]
fn c4_4_build_redis_config_both_none() {
    let config = build_redis_config(None, None);
    assert_eq!(config, None);
}

// ============================================================
// TC-C5: build_telegram_config
// ============================================================

#[test]
fn c5_1_build_telegram_config_both_present() {
    let config = build_telegram_config(
        Some("bot_token_123".into()),
        Some("chat_id_456".into()),
    );
    assert!(config.is_some());
    let tg = config.unwrap();
    assert_eq!(tg.bot_token, "bot_token_123");
    assert_eq!(tg.chat_id, "chat_id_456");
}

#[test]
fn c5_2_build_telegram_config_missing_chat_id() {
    let config = build_telegram_config(Some("bot_token_123".into()), None);
    assert_eq!(config, None);
}

#[test]
fn c5_3_build_telegram_config_missing_token() {
    let config = build_telegram_config(None, Some("chat_id_456".into()));
    assert_eq!(config, None);
}

#[test]
fn c5_4_build_telegram_config_both_none() {
    let config = build_telegram_config(None, None);
    assert_eq!(config, None);
}

// ============================================================
// TC-C6: build_email_config
// ============================================================

#[test]
fn c6_1_build_email_config_all_fields() {
    let config = build_email_config(
        Some("smtp.example.com".into()),
        Some("user@example.com".into()),
        Some("password".into()),
        Some("465".into()),
        Some("custom@example.com".into()),
    );
    assert!(config.is_some());
    let email = config.unwrap();
    assert_eq!(email.host, "smtp.example.com");
    assert_eq!(email.port, 465);
    assert_eq!(email.username, "user@example.com");
    assert_eq!(email.password, "password");
    assert_eq!(email.from, "custom@example.com");
}

#[test]
fn c6_2_build_email_config_defaults_for_port_and_from() {
    let config = build_email_config(
        Some("smtp.example.com".into()),
        Some("user@example.com".into()),
        Some("password".into()),
        None,
        None,
    );
    assert!(config.is_some());
    let email = config.unwrap();
    assert_eq!(email.port, 587);
    assert_eq!(email.from, "noreply@virs.com");
}

#[test]
fn c6_3_build_email_config_no_host() {
    let config = build_email_config(
        None,
        Some("user@example.com".into()),
        Some("password".into()),
        None,
        None,
    );
    assert_eq!(config, None);
}

#[test]
fn c6_4_build_email_config_no_username() {
    let config = build_email_config(
        Some("smtp.example.com".into()),
        None,
        Some("password".into()),
        None,
        None,
    );
    assert_eq!(config, None);
}

#[test]
fn c6_5_build_email_config_no_password() {
    let config = build_email_config(
        Some("smtp.example.com".into()),
        Some("user@example.com".into()),
        None,
        None,
        None,
    );
    assert_eq!(config, None);
}

#[test]
fn c6_6_build_email_config_invalid_port_uses_default() {
    let config = build_email_config(
        Some("smtp.example.com".into()),
        Some("user@example.com".into()),
        Some("password".into()),
        Some("not_a_number".into()),
        None,
    );
    assert!(config.is_some());
    let email = config.unwrap();
    assert_eq!(email.port, 587);
}

// ============================================================
// TC-S3: Default value constants verification
// ============================================================

#[test]
fn s3_1_default_constants_values() {
    assert_eq!(DEFAULT_HOST, "0.0.0.0");
    assert_eq!(DEFAULT_PORT, "8080");
    assert_eq!(DEFAULT_LOG_LEVEL, "info");
    assert_eq!(DEFAULT_JWT_HOURS, "24");
    assert_eq!(DEFAULT_DB_POOL_MIN, "5");
    assert_eq!(DEFAULT_DB_POOL_MAX, "50");
    assert_eq!(DEFAULT_SMTP_PORT, "587");
    assert_eq!(DEFAULT_SMTP_FROM, "noreply@virs.com");
    assert_eq!(DEFAULT_CACHE_TTL_TICKER, "10");
    assert_eq!(DEFAULT_CACHE_TTL_KLINE_1M, "60");
    assert_eq!(DEFAULT_CACHE_TTL_KLINE_5M, "120");
    assert_eq!(DEFAULT_CACHE_TTL_KLINE_1H, "300");
    assert_eq!(DEFAULT_CACHE_TTL_KLINE_1D, "3600");
    // NOTE: ADMIN_USERNAME and ADMIN_PASSWORD no longer have defaults.
    // They must be provided via environment variables — see app_config.rs.
}
