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
            secret_key: "secret".into(),
            encryption_key: "enc_key".into(),
            jwt_expiration_hours: 24,
        },
        database: DatabaseConfig {
            url: "postgres://localhost/virs".into(),
            pool_min: 5,
            pool_max: 50,
        },
        admin: AdminConfig {
            username: "admin".into(),
            password: "admin123".into(),
            id: Some(uuid::Uuid::nil()),
        },
        proxy: Some("http://proxy:8080".into()),
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: AppConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.server.host, config.server.host);
    assert_eq!(deserialized.server.port, config.server.port);
    assert_eq!(deserialized.server.secret_key, config.server.secret_key);
    assert_eq!(deserialized.database.url, config.database.url);
    assert_eq!(deserialized.database.pool_min, config.database.pool_min);
    assert_eq!(deserialized.database.pool_max, config.database.pool_max);
    assert_eq!(deserialized.admin.username, config.admin.username);
    assert_eq!(deserialized.admin.id, config.admin.id);
    assert_eq!(deserialized.proxy, config.proxy);
}

// ============================================================
// TC-S2: Sub-config serde round-trips
// ============================================================

#[test]
fn s2_1_server_config_roundtrip() {
    let config = ServerConfig {
        host: "127.0.0.1".into(),
        port: 3000,
        secret_key: "my_secret".into(),
        encryption_key: "my_enc_key".into(),
        jwt_expiration_hours: 48,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: ServerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.host, config.host);
    assert_eq!(de.port, config.port);
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
