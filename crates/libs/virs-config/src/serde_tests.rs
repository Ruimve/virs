//! Serde serialization/deserialization round-trip tests for all config types.

use crate::app_config::*;

// ============================================================
// TC-S1: AppConfig serde round-trip
// ============================================================

#[test]
fn s1_1_app_config_full_roundtrip() {
    let config = AppConfig {
        server: ServerConfig {
            host: "0.0.0.0".into(),
            port: 8080,
            log_level: "info".into(),
            secret_key: "secret".into(),
            encryption_key: "enc_key".into(),
            jwt_expiration_hours: 24,
        },
        database: DatabaseConfig {
            url: "postgres://localhost/virs".into(),
            pool_min: 5,
            pool_max: 50,
        },
        redis: Some(RedisConfig {
            url: "redis://localhost:6379".into(),
            password: Some("redis_pass".into()),
        }),
        admin: AdminConfig {
            username: "admin".into(),
            password: "admin123".into(),
            id: Some(uuid::Uuid::nil()),
        },
        ai: AiConfig {
            openrouter_api_key: Some("or_key".into()),
            openai_api_key: None,
            deepseek_api_key: Some("ds_key".into()),
        },
        notification: NotificationConfig {
            telegram: Some(TelegramConfig {
                bot_token: "bot_token".into(),
                chat_id: "chat_id".into(),
            }),
            email: Some(EmailConfig {
                host: "smtp.example.com".into(),
                port: 587,
                username: "user".into(),
                password: "pass".into(),
                from: "noreply@virs.com".into(),
            }),
        },
        cache: CacheConfig {
            ttl_ticker: 10,
            ttl_kline_1m: 60,
            ttl_kline_5m: 120,
            ttl_kline_1h: 300,
            ttl_kline_1d: 3600,
        },
        proxy: Some("http://proxy:8080".into()),
        paper: Some(false),
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: AppConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.server.host, config.server.host);
    assert_eq!(deserialized.server.port, config.server.port);
    assert_eq!(deserialized.server.secret_key, config.server.secret_key);
    assert_eq!(deserialized.database.url, config.database.url);
    assert_eq!(deserialized.database.pool_min, config.database.pool_min);
    assert_eq!(deserialized.database.pool_max, config.database.pool_max);
    assert_eq!(deserialized.redis.as_ref().unwrap().url, config.redis.as_ref().unwrap().url);
    assert_eq!(deserialized.redis.as_ref().unwrap().password, config.redis.as_ref().unwrap().password);
    assert_eq!(deserialized.admin.username, config.admin.username);
    assert_eq!(deserialized.admin.id, config.admin.id);
    assert_eq!(deserialized.ai.openrouter_api_key, config.ai.openrouter_api_key);
    assert_eq!(deserialized.ai.openai_api_key, config.ai.openai_api_key);
    assert_eq!(deserialized.notification.telegram.as_ref().unwrap().bot_token,
               config.notification.telegram.as_ref().unwrap().bot_token);
    assert_eq!(deserialized.notification.email.as_ref().unwrap().host,
               config.notification.email.as_ref().unwrap().host);
    assert_eq!(deserialized.cache.ttl_ticker, config.cache.ttl_ticker);
    assert_eq!(deserialized.cache.ttl_kline_1d, config.cache.ttl_kline_1d);
    assert_eq!(deserialized.proxy, config.proxy);
    assert_eq!(deserialized.paper, config.paper);
}

#[test]
fn s1_2_app_config_paper_missing_uses_default() {
    // JSON without "paper" field should default to Some(true)
    let json = r#"{
        "server": {
            "host": "0.0.0.0",
            "port": 8080,
            "log_level": "info",
            "secret_key": "s",
            "encryption_key": "e",
            "jwt_expiration_hours": 24
        },
        "database": {
            "url": "postgres://localhost/db",
            "pool_min": 5,
            "pool_max": 50
        },
        "admin": {
            "username": "admin",
            "password": "admin123",
            "id": null
        },
        "ai": {
            "openrouter_api_key": null,
            "openai_api_key": null,
            "deepseek_api_key": null
        },
        "notification": {
            "telegram": null,
            "email": null
        },
        "cache": {
            "ttl_ticker": 10,
            "ttl_kline_1m": 60,
            "ttl_kline_5m": 120,
            "ttl_kline_1h": 300,
            "ttl_kline_1d": 3600
        },
        "proxy": null
    }"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.paper, Some(true));
}

// ============================================================
// TC-S2: Sub-config serde round-trips
// ============================================================

#[test]
fn s2_1_server_config_roundtrip() {
    let config = ServerConfig {
        host: "127.0.0.1".into(),
        port: 3000,
        log_level: "debug".into(),
        secret_key: "my_secret".into(),
        encryption_key: "my_enc_key".into(),
        jwt_expiration_hours: 48,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: ServerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.host, config.host);
    assert_eq!(de.port, config.port);
    assert_eq!(de.log_level, config.log_level);
    assert_eq!(de.secret_key, config.secret_key);
    assert_eq!(de.encryption_key, config.encryption_key);
    assert_eq!(de.jwt_expiration_hours, config.jwt_expiration_hours);
}

#[test]
fn s2_2_database_config_roundtrip() {
    let config = DatabaseConfig {
        url: "postgres://user:pass@localhost:5432/virs".into(),
        pool_min: 10,
        pool_max: 100,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: DatabaseConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.url, config.url);
    assert_eq!(de.pool_min, config.pool_min);
    assert_eq!(de.pool_max, config.pool_max);
}

#[test]
fn s2_3_redis_config_roundtrip_with_none_password() {
    let config = RedisConfig {
        url: "redis://localhost:6379".into(),
        password: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: RedisConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.url, config.url);
    assert_eq!(de.password, None);
}

#[test]
fn s2_4_admin_config_roundtrip_with_none_id() {
    let config = AdminConfig {
        username: "admin".into(),
        password: "pass".into(),
        id: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: AdminConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.username, config.username);
    assert_eq!(de.password, config.password);
    assert_eq!(de.id, None);
}

#[test]
fn s2_5_cache_config_roundtrip() {
    let config = CacheConfig {
        ttl_ticker: 10,
        ttl_kline_1m: 60,
        ttl_kline_5m: 120,
        ttl_kline_1h: 300,
        ttl_kline_1d: 3600,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: CacheConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.ttl_ticker, config.ttl_ticker);
    assert_eq!(de.ttl_kline_1m, config.ttl_kline_1m);
    assert_eq!(de.ttl_kline_5m, config.ttl_kline_5m);
    assert_eq!(de.ttl_kline_1h, config.ttl_kline_1h);
    assert_eq!(de.ttl_kline_1d, config.ttl_kline_1d);
}

#[test]
fn s2_6_telegram_config_roundtrip() {
    let config = TelegramConfig {
        bot_token: "123456:ABC-DEF".into(),
        chat_id: "987654321".into(),
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: TelegramConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.bot_token, config.bot_token);
    assert_eq!(de.chat_id, config.chat_id);
}

#[test]
fn s2_7_email_config_roundtrip() {
    let config = EmailConfig {
        host: "smtp.gmail.com".into(),
        port: 465,
        username: "user@gmail.com".into(),
        password: "app_password".into(),
        from: "user@gmail.com".into(),
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: EmailConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.host, config.host);
    assert_eq!(de.port, config.port);
    assert_eq!(de.username, config.username);
    assert_eq!(de.password, config.password);
    assert_eq!(de.from, config.from);
}
