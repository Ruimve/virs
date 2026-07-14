use std::sync::Mutex;

use virs_config::{load_config, load_config_from_env};


static ENV_LOCK: Mutex<()> = Mutex::new(());


fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}


fn set_required_env_vars() {
    std::env::set_var("ENCRYPTION_KEY", "test_encryption_key");
    std::env::set_var("LLM_KEY", "test_llm_key");
    std::env::set_var("DATABASE_URL", "postgres://localhost/virs_test");


    std::env::set_var("ADMIN_USERNAME", "test_admin");
    std::env::set_var("ADMIN_PASSWORD", "test_password_at_least_12_chars");

    std::env::set_var("JWT_SECRET", "test_jwt_secret_at_least_32_chars_long");
}


fn clean_env_vars() {
    let keys = [
        "ENCRYPTION_KEY", "LLM_KEY", "DATABASE_URL",
        "HOST", "PORT", "JWT_EXPIRATION_HOURS", "JWT_SECRET",
        "DB_POOL_MIN", "DB_POOL_MAX", "DB_ACQUIRE_TIMEOUT_SECS",
        "ADMIN_USERNAME", "ADMIN_PASSWORD",
        "PROXY_URL",
        "TIME_MAX_POSITION_DURATION_SECS",
        "TIME_PENDING_ORDER_TIMEOUT_SECS",
        "TIME_PRICE_POLL_INTERVAL_SECS",
        "TIME_CLOSE_ORDER_TIMEOUT_SECS",
        "TIME_HTTP_TIMEOUT_SECS",
        "TIME_LLM_TIMEOUT_SECS",
        "INITIAL_PRICE_MAX_RETRIES",
        "PERSIST_MAX_RETRIES",
        "PERSIST_RETRY_BASE_MS",
        "HTTP_CONNECT_TIMEOUT_SECS",
        "HTTP_POOL_MAX_IDLE_PER_HOST",
        "WS_RECONNECT_INITIAL_DELAY_SECS",
        "WS_RECONNECT_MAX_DELAY_SECS",
        "WS_PING_INTERVAL_SECS",
        "WS_MAX_LIFETIME_SECS",
        "LISTENKEY_KEEPALIVE_FUTURES_SECS",
    ];
    for key in &keys {
        std::env::remove_var(key);
    }
}


#[test]
fn int_1_1_load_config_minimal_required() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();

    let config = load_config();
    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.server.encryption_key, "test_encryption_key");
    assert_eq!(config.server.llm_key, "test_llm_key");
    assert_eq!(config.database.url, "postgres://localhost/virs_test");
}

#[test]
fn int_1_2_load_config_missing_llm_key() {
    let _guard = lock_env();
    clean_env_vars();
    std::env::set_var("ENCRYPTION_KEY", "enc");
    std::env::set_var("DATABASE_URL", "postgres://localhost/db");

    let result = load_config_from_env();
    assert!(result.is_err());
    assert!(result.err().unwrap().to_string().contains("LLM_KEY"));
}

#[test]
fn int_1_3_load_config_missing_encryption_key() {
    let _guard = lock_env();
    clean_env_vars();
    std::env::set_var("LLM_KEY", "llm_key");
    std::env::set_var("DATABASE_URL", "postgres://localhost/db");

    let result = load_config_from_env();
    assert!(result.is_err());
    assert!(result.err().unwrap().to_string().contains("ENCRYPTION_KEY"));
}

#[test]
fn int_1_4_load_config_missing_database_url() {
    let _guard = lock_env();
    clean_env_vars();
    std::env::set_var("ENCRYPTION_KEY", "enc");
    std::env::set_var("LLM_KEY", "llm_key");
    std::env::set_var("JWT_SECRET", "test_jwt_secret_at_least_32_chars_long");

    let result = load_config_from_env();
    assert!(result.is_err());
    assert!(result.err().unwrap().to_string().contains("DATABASE_URL"));
}


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


#[test]
fn int_6_1_same_encryption_and_llm_key_rejected() {
    let _guard = lock_env();
    clean_env_vars();
    std::env::set_var("ENCRYPTION_KEY", "same_key_value");
    std::env::set_var("LLM_KEY", "same_key_value");
    std::env::set_var("DATABASE_URL", "postgres://localhost/db");
    std::env::set_var("ADMIN_USERNAME", "admin");
    std::env::set_var("ADMIN_PASSWORD", "password_at_least_12_chars");
    std::env::set_var("JWT_SECRET", "jwt_secret_at_least_32_chars_long");

    let result = load_config_from_env();
    assert!(result.is_err());
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("ENCRYPTION_KEY and LLM_KEY must be different"),
        "Expected key collision error, got: {}", err_msg
    );
}

#[test]
fn int_6_2_different_encryption_and_llm_key_accepted() {
    let _guard = lock_env();
    clean_env_vars();
    std::env::set_var("ENCRYPTION_KEY", "encryption_key_value");
    std::env::set_var("LLM_KEY", "different_llm_key_value");
    std::env::set_var("DATABASE_URL", "postgres://localhost/db");
    std::env::set_var("ADMIN_USERNAME", "admin");
    std::env::set_var("ADMIN_PASSWORD", "password_at_least_12_chars");
    std::env::set_var("JWT_SECRET", "jwt_secret_at_least_32_chars_long");

    let result = load_config_from_env();
    assert!(result.is_ok());
}
