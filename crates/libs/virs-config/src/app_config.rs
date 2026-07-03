use serde::{Deserialize, Serialize};
use std::str::FromStr;
use virs_error::{VirsError, VirsResult};

// ============================================================
// Default value constants
// ============================================================

pub(crate) const DEFAULT_HOST: &str = "0.0.0.0";
pub(crate) const DEFAULT_PORT: &str = "8080";
pub(crate) const DEFAULT_JWT_HOURS: &str = "24";
pub(crate) const DEFAULT_DB_POOL_MIN: &str = "5";
pub(crate) const DEFAULT_DB_POOL_MAX: &str = "50";
// ADMIN_USERNAME and ADMIN_PASSWORD have NO defaults — they must be set
// explicitly via environment variables. This is a security requirement:
// hard-coded credentials allow attackers to forge admin access.

// ============================================================
// Pure parsing functions (idempotent, no side effects)
// ============================================================

/// Parse an optional environment variable string into a numeric type, using a default when absent.
///
/// Returns an error if the value is present but cannot be parsed.
pub(crate) fn parse_env_num<T: FromStr>(value: Option<String>, default: &str) -> VirsResult<T>
where
    <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    let s = value.unwrap_or_else(|| default.to_string());
    s.parse::<T>()
        .map_err(|e| VirsError::config(format!("Failed to parse '{}': {}", s, e)))
}

// ============================================================
// Configuration structs
// ============================================================

/// Master application configuration, loaded from environment variables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub admin: AdminConfig,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub secret_key: String,
    pub encryption_key: String,
    pub jwt_expiration_hours: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub pool_min: u32,
    pub pool_max: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminConfig {
    pub username: String,
    pub password: String,
    /// UUID of the initial admin user, set at startup after creation/lookup.
    pub id: Option<uuid::Uuid>,
}

// ============================================================
// Load configuration from environment variables
// ============================================================

/// Load configuration from environment variables.
/// Falls back to sensible defaults for optional fields.
///
/// This function first loads `.env` via dotenvy, then reads all variables from
/// the process environment. To test config loading without `.env` interference,
/// use [`load_config_from_env`] directly.
pub fn load_config() -> VirsResult<AppConfig> {
    dotenvy::dotenv().ok();
    load_config_from_env()
}

/// Load configuration from environment variables without loading `.env` file.
///
/// This is the pure env-reading portion of [`load_config`], extracted for
/// testability. All defaults and validation logic are identical.
pub fn load_config_from_env() -> VirsResult<AppConfig> {

    let server = ServerConfig {
        host: std::env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.into()),
        port: parse_env_num(std::env::var("PORT").ok(), DEFAULT_PORT)?,
        secret_key: std::env::var("SECRET_KEY").map_err(|_| {
            VirsError::config("SECRET_KEY environment variable is required")
        })?,
        encryption_key: std::env::var("ENCRYPTION_KEY").map_err(|_| {
            VirsError::config(
                "ENCRYPTION_KEY environment variable is required (must differ from SECRET_KEY)",
            )
        })?,
        jwt_expiration_hours: parse_env_num(std::env::var("JWT_EXPIRATION_HOURS").ok(), DEFAULT_JWT_HOURS)?,
    };

    let database = DatabaseConfig {
        url: std::env::var("DATABASE_URL").map_err(|_| {
            VirsError::config("DATABASE_URL environment variable is required")
        })?,
        pool_min: parse_env_num(std::env::var("DB_POOL_MIN").ok(), DEFAULT_DB_POOL_MIN)?,
        pool_max: parse_env_num(std::env::var("DB_POOL_MAX").ok(), DEFAULT_DB_POOL_MAX)?,
    };

    let admin = AdminConfig {
        username: std::env::var("ADMIN_USERNAME").map_err(|_| {
            VirsError::config("ADMIN_USERNAME environment variable is required")
        })?,
        password: {
            let pwd = std::env::var("ADMIN_PASSWORD").map_err(|_| {
                VirsError::config("ADMIN_PASSWORD environment variable is required")
            })?;
            if pwd.len() < 12 {
                return Err(VirsError::config(
                    "ADMIN_PASSWORD must be at least 12 characters for security",
                ));
            }
            pwd
        },
        id: None,
    };

    let proxy = std::env::var("PROXY_URL").ok();

    Ok(AppConfig {
        server,
        database,
        admin,
        proxy,
    })
}
