//! Integration tests for virs-config.
//!
//! Tests load_config() end-to-end with controlled environment variables,
//! and cross-module config construction pipelines.

use std::sync::Mutex;

use virs_config::{
    load_config, load_config_from_env, AdminConfig, AppConfig, DatabaseConfig,
    ServerConfig,
};

/// Mutex to serialize tests that modify environment variables (env is process-global).
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Helper: acquire the env lock, recovering from poisoned state if a prior test panicked.
fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Helper: set required env vars for load_config to succeed.
fn set_required_env_vars() {
    std::env::set_var("SECRET_KEY", "test_secret_key");
    std::env::set_var("ENCRYPTION_KEY", "test_encryption_key");
    std::env::set_var("DATABASE_URL", "postgres://localhost/virs_test");
    // ADMIN_USERNAME and ADMIN_PASSWORD are required (no defaults).
    // ADMIN_PASSWORD must be at least 12 characters.
    std::env::set_var("ADMIN_USERNAME", "test_admin");
    std::env::set_var("ADMIN_PASSWORD", "test_password_at_least_12_chars");
    // JWT_SECRET is required for token signing/verification.
    std::env::set_var("JWT_SECRET", "test_jwt_secret_at_least_32_chars_long");
}

/// Helper: remove all config-related env vars to ensure a clean state.
fn clean_env_vars() {
    let keys = [
        "SECRET_KEY", "ENCRYPTION_KEY", "DATABASE_URL",
        "HOST", "PORT", "JWT_EXPIRATION_HOURS", "JWT_SECRET",
        "DB_POOL_MIN", "DB_POOL_MAX",
        "ADMIN_USERNAME", "ADMIN_PASSWORD",
        "PROXY_URL",
    ];
    for key in &keys {
        std::env::remove_var(key);
    }
}

// ============================================================
// TC-INT-1: load_config minimal required env vars
// ============================================================

#[test]
fn int_1_1_load_config_minimal_required() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();

    let config = load_config();
    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.server.secret_key, "test_secret_key");
    assert_eq!(config.server.encryption_key, "test_encryption_key");
    assert_eq!(config.database.url, "postgres://localhost/virs_test");
}

#[test]
fn int_1_2_load_config_missing_secret_key() {
    let _guard = lock_env();
    clean_env_vars();
    std::env::set_var("ENCRYPTION_KEY", "enc");
    std::env::set_var("DATABASE_URL", "postgres://localhost/db");

    let result = load_config_from_env();
    assert!(result.is_err());
    assert!(result.err().unwrap().to_string().contains("SECRET_KEY"));
}

#[test]
fn int_1_3_load_config_missing_encryption_key() {
    let _guard = lock_env();
    clean_env_vars();
    std::env::set_var("SECRET_KEY", "secret");
    std::env::set_var("DATABASE_URL", "postgres://localhost/db");

    let result = load_config_from_env();
    assert!(result.is_err());
    assert!(result.err().unwrap().to_string().contains("ENCRYPTION_KEY"));
}

#[test]
fn int_1_4_load_config_missing_database_url() {
    let _guard = lock_env();
    clean_env_vars();
    std::env::set_var("SECRET_KEY", "secret");
    std::env::set_var("ENCRYPTION_KEY", "enc");
    std::env::set_var("JWT_SECRET", "test_jwt_secret_at_least_32_chars_long");

    let result = load_config_from_env();
    assert!(result.is_err());
    assert!(result.err().unwrap().to_string().contains("DATABASE_URL"));
}

// ============================================================
// TC-INT-2: load_config default values
// ============================================================

#[test]
fn int_2_1_default_port() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();

    let config = load_config().unwrap();
    assert_eq!(config.server.port, 8080);
}

#[test]
fn int_2_2_default_host() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();

    let config = load_config().unwrap();
    assert_eq!(config.server.host, "0.0.0.0");
}

#[test]
fn int_2_4_default_jwt_hours() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();

    let config = load_config().unwrap();
    assert_eq!(config.server.jwt_expiration_hours, 24);
}

#[test]
fn int_2_5_default_db_pool() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();

    let config = load_config().unwrap();
    assert_eq!(config.database.pool_min, 5);
    assert_eq!(config.database.pool_max, 50);
}

// ============================================================
// TC-INT-5: load_config custom values
// ============================================================

#[test]
fn int_5_1_custom_port() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("PORT", "3000");

    let config = load_config().unwrap();
    assert_eq!(config.server.port, 3000);
}

#[test]
fn int_5_2_custom_db_pool_max() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("DB_POOL_MAX", "100");

    let config = load_config().unwrap();
    assert_eq!(config.database.pool_max, 100);
}

#[test]
fn int_5_4_custom_proxy() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("PROXY_URL", "http://proxy:8080");

    let config = load_config().unwrap();
    assert_eq!(config.proxy, Some("http://proxy:8080".into()));
}
