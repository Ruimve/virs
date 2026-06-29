//! Integration tests for virs-config.
//!
//! Tests load_config() end-to-end with controlled environment variables,
//! and cross-module config construction pipelines.

use std::sync::Mutex;

use virs_config::{
    load_config, load_config_from_env, AiConfig, AdminConfig, AppConfig, CacheConfig, DatabaseConfig,
    EmailConfig, NotificationConfig, RedisConfig, ServerConfig, TelegramConfig,
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
}

/// Helper: remove all config-related env vars to ensure a clean state.
fn clean_env_vars() {
    let keys = [
        "SECRET_KEY", "ENCRYPTION_KEY", "DATABASE_URL",
        "HOST", "PORT", "LOG_LEVEL", "JWT_EXPIRATION_HOURS",
        "DB_POOL_MIN", "DB_POOL_MAX",
        "REDIS_URL", "REDIS_PASSWORD",
        "ADMIN_USERNAME", "ADMIN_PASSWORD",
        "OPENROUTER_API_KEY", "OPENAI_API_KEY", "DEEPSEEK_API_KEY",
        "TELEGRAM_BOT_TOKEN", "TELEGRAM_CHAT_ID",
        "SMTP_HOST", "SMTP_USERNAME", "SMTP_PASSWORD", "SMTP_PORT", "SMTP_FROM",
        "CACHE_TTL_TICKER", "CACHE_TTL_KLINE_1M", "CACHE_TTL_KLINE_5M",
        "CACHE_TTL_KLINE_1H", "CACHE_TTL_KLINE_1D",
        "PROXY_URL", "PAPER_TRADING",
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
fn int_2_3_default_log_level() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();

    let config = load_config().unwrap();
    assert_eq!(config.server.log_level, "info");
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
// TC-INT-3: load_config optional configs
// ============================================================

#[test]
fn int_3_1_redis_with_password() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("REDIS_URL", "redis://localhost:6379");
    std::env::set_var("REDIS_PASSWORD", "redis_secret");

    let config = load_config().unwrap();
    let redis = config.redis.unwrap();
    assert_eq!(redis.url, "redis://localhost:6379");
    assert_eq!(redis.password, Some("redis_secret".into()));
}

#[test]
fn int_3_2_redis_not_set() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();

    let config = load_config_from_env().unwrap();
    assert!(config.redis.is_none());
}

#[test]
fn int_3_3_telegram_both_set() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("TELEGRAM_BOT_TOKEN", "bot_token_abc");
    std::env::set_var("TELEGRAM_CHAT_ID", "chat_123");

    let config = load_config().unwrap();
    let tg = config.notification.telegram.unwrap();
    assert_eq!(tg.bot_token, "bot_token_abc");
    assert_eq!(tg.chat_id, "chat_123");
}

#[test]
fn int_3_4_telegram_only_token() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("TELEGRAM_BOT_TOKEN", "bot_token_abc");

    let config = load_config_from_env().unwrap();
    assert!(config.notification.telegram.is_none());
}

#[test]
fn int_3_5_email_all_required_set() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("SMTP_HOST", "smtp.example.com");
    std::env::set_var("SMTP_USERNAME", "user@example.com");
    std::env::set_var("SMTP_PASSWORD", "email_pass");

    let config = load_config().unwrap();
    let email = config.notification.email.unwrap();
    assert_eq!(email.host, "smtp.example.com");
    assert_eq!(email.username, "user@example.com");
    assert_eq!(email.password, "email_pass");
    assert_eq!(email.port, 587);
    assert_eq!(email.from, "noreply@virs.com");
}

#[test]
fn int_3_6_email_custom_port() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("SMTP_HOST", "smtp.example.com");
    std::env::set_var("SMTP_USERNAME", "user@example.com");
    std::env::set_var("SMTP_PASSWORD", "email_pass");
    std::env::set_var("SMTP_PORT", "465");

    let config = load_config().unwrap();
    let email = config.notification.email.unwrap();
    assert_eq!(email.port, 465);
}

// ============================================================
// TC-INT-4: load_config paper trading
// ============================================================

#[test]
fn int_4_1_paper_trading_true() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("PAPER_TRADING", "true");

    let config = load_config().unwrap();
    assert_eq!(config.paper, Some(true));
}

#[test]
fn int_4_2_paper_trading_one() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("PAPER_TRADING", "1");

    let config = load_config().unwrap();
    assert_eq!(config.paper, Some(true));
}

#[test]
fn int_4_3_paper_trading_false() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("PAPER_TRADING", "false");

    let config = load_config().unwrap();
    assert_eq!(config.paper, Some(false));
}

#[test]
fn int_4_4_paper_trading_not_set_defaults_true() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();

    let config = load_config().unwrap();
    assert_eq!(config.paper, Some(true));
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
fn int_5_3_custom_cache_ttl() {
    let _guard = lock_env();
    clean_env_vars();
    set_required_env_vars();
    std::env::set_var("CACHE_TTL_TICKER", "30");

    let config = load_config().unwrap();
    assert_eq!(config.cache.ttl_ticker, 30);
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

// ============================================================
// TC-INT-6: Config construction → AppConfig → serde round-trip
// ============================================================

#[test]
fn int_6_1_redis_config_in_appconfig_roundtrip() {
    let redis = RedisConfig {
        url: "redis://localhost:6379".into(),
        password: Some("secret".into()),
    };

    let config = AppConfig {
        server: ServerConfig {
            host: "0.0.0.0".into(), port: 8080, log_level: "info".into(),
            secret_key: "s".into(), encryption_key: "e".into(), jwt_expiration_hours: 24,
        },
        database: DatabaseConfig {
            url: "postgres://localhost/db".into(), pool_min: 5, pool_max: 50,
        },
        redis: Some(redis),
        admin: AdminConfig { username: "admin".into(), password: "pass".into(), id: None },
        ai: AiConfig { openrouter_api_key: None, openai_api_key: None, deepseek_api_key: None },
        notification: NotificationConfig { telegram: None, email: None },
        cache: CacheConfig {
            ttl_ticker: 10, ttl_kline_1m: 60, ttl_kline_5m: 120, ttl_kline_1h: 300, ttl_kline_1d: 3600,
        },
        proxy: None,
        paper: Some(true),
    };

    let json = serde_json::to_string(&config).unwrap();
    let de: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.redis, config.redis);
}

#[test]
fn int_6_2_notification_config_in_appconfig_roundtrip() {
    let telegram = TelegramConfig {
        bot_token: "bot_token".into(),
        chat_id: "chat_id".into(),
    };
    let email = EmailConfig {
        host: "smtp.example.com".into(),
        port: 465,
        username: "user".into(),
        password: "pass".into(),
        from: "from@example.com".into(),
    };

    let config = AppConfig {
        server: ServerConfig {
            host: "0.0.0.0".into(), port: 8080, log_level: "info".into(),
            secret_key: "s".into(), encryption_key: "e".into(), jwt_expiration_hours: 24,
        },
        database: DatabaseConfig {
            url: "postgres://localhost/db".into(), pool_min: 5, pool_max: 50,
        },
        redis: None,
        admin: AdminConfig { username: "admin".into(), password: "pass".into(), id: None },
        ai: AiConfig { openrouter_api_key: None, openai_api_key: None, deepseek_api_key: None },
        notification: NotificationConfig {
            telegram: Some(telegram),
            email: Some(email),
        },
        cache: CacheConfig {
            ttl_ticker: 10, ttl_kline_1m: 60, ttl_kline_5m: 120, ttl_kline_1h: 300, ttl_kline_1d: 3600,
        },
        proxy: None,
        paper: Some(false),
    };

    let json = serde_json::to_string(&config).unwrap();
    let de: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.notification.telegram, config.notification.telegram);
    assert_eq!(de.notification.email, config.notification.email);
    assert_eq!(de.paper, Some(false));
}
