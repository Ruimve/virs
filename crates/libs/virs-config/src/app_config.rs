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

// T12: TimeConfig default constants
pub(crate) const DEFAULT_MAX_POSITION_DURATION_SECS: &str = "172800"; // 48h
pub(crate) const DEFAULT_PENDING_ORDER_TIMEOUT_SECS: &str = "60";
pub(crate) const DEFAULT_PRICE_POLL_INTERVAL_SECS: &str = "5";
pub(crate) const DEFAULT_CLOSE_ORDER_TIMEOUT_SECS: &str = "15";
pub(crate) const DEFAULT_HTTP_TIMEOUT_SECS: &str = "30";
pub(crate) const DEFAULT_LLM_TIMEOUT_SECS: &str = "120";

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
    pub time: TimeConfig,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub encryption_key: String,
    pub llm_key: String,
    pub jwt_secret: String,
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

/// T12: Time-related configuration extracted from hardcoded constants.
///
/// All values are loaded from environment variables with safe defaults.
/// On initialization, `warn!` is logged to ensure observability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeConfig {
    /// 最大持仓时长（秒）— 超过此值强制平仓。默认 48h = 172800s
    pub max_position_duration_secs: u64,
    /// 挂单超时时间（秒）— 超过此值取消挂单。默认 60s
    pub pending_order_timeout_secs: u64,
    /// 价格轮询间隔（秒）— auto worker 定时查询价格。默认 5s
    pub price_poll_interval_secs: u64,
    /// 平仓订单等待超时（秒）— 等待 PE 仓位事件恢复。默认 15s
    pub close_order_timeout_secs: u64,
    /// HTTP 请求超时（秒）— 交易所 REST API 调用。默认 30s
    pub http_timeout_secs: u64,
    /// LLM 请求超时（秒）— AI 决策调用。默认 120s
    pub llm_timeout_secs: u64,
}

impl Default for TimeConfig {
    fn default() -> Self {
        tracing::warn!(
            "TimeConfig::default() called — using default time parameters. \
             Set TIME_* env vars to override."
        );
        Self {
            max_position_duration_secs: DEFAULT_MAX_POSITION_DURATION_SECS.parse().unwrap(),
            pending_order_timeout_secs: DEFAULT_PENDING_ORDER_TIMEOUT_SECS.parse().unwrap(),
            price_poll_interval_secs: DEFAULT_PRICE_POLL_INTERVAL_SECS.parse().unwrap(),
            close_order_timeout_secs: DEFAULT_CLOSE_ORDER_TIMEOUT_SECS.parse().unwrap(),
            http_timeout_secs: DEFAULT_HTTP_TIMEOUT_SECS.parse().unwrap(),
            llm_timeout_secs: DEFAULT_LLM_TIMEOUT_SECS.parse().unwrap(),
        }
    }
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
        encryption_key: std::env::var("ENCRYPTION_KEY").map_err(|_| {
            VirsError::config("ENCRYPTION_KEY environment variable is required")
        })?,
        llm_key: std::env::var("LLM_KEY").map_err(|_| {
            VirsError::config("LLM_KEY environment variable is required")
        })?,
        jwt_secret: std::env::var("JWT_SECRET").map_err(|_| {
            VirsError::config("JWT_SECRET environment variable is required")
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

    let proxy = std::env::var("PROXY_URL")
        .ok()
        .filter(|s| !s.trim().is_empty());

    // T12: Load time configuration from env vars
    let time = TimeConfig {
        max_position_duration_secs: parse_env_num(
            std::env::var("TIME_MAX_POSITION_DURATION_SECS").ok(),
            DEFAULT_MAX_POSITION_DURATION_SECS,
        )?,
        pending_order_timeout_secs: parse_env_num(
            std::env::var("TIME_PENDING_ORDER_TIMEOUT_SECS").ok(),
            DEFAULT_PENDING_ORDER_TIMEOUT_SECS,
        )?,
        price_poll_interval_secs: parse_env_num(
            std::env::var("TIME_PRICE_POLL_INTERVAL_SECS").ok(),
            DEFAULT_PRICE_POLL_INTERVAL_SECS,
        )?,
        close_order_timeout_secs: parse_env_num(
            std::env::var("TIME_CLOSE_ORDER_TIMEOUT_SECS").ok(),
            DEFAULT_CLOSE_ORDER_TIMEOUT_SECS,
        )?,
        http_timeout_secs: parse_env_num(
            std::env::var("TIME_HTTP_TIMEOUT_SECS").ok(),
            DEFAULT_HTTP_TIMEOUT_SECS,
        )?,
        llm_timeout_secs: parse_env_num(
            std::env::var("TIME_LLM_TIMEOUT_SECS").ok(),
            DEFAULT_LLM_TIMEOUT_SECS,
        )?,
    };

    Ok(AppConfig {
        server,
        database,
        admin,
        time,
        proxy,
    })
}
