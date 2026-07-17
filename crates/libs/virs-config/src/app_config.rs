use serde::{Deserialize, Serialize};
use std::str::FromStr;
use virs_error::{VirsError, VirsResult};

pub(crate) const DEFAULT_HOST: &str = "0.0.0.0";
pub(crate) const DEFAULT_PORT: &str = "8080";
pub(crate) const DEFAULT_JWT_HOURS: &str = "24";
pub(crate) const DEFAULT_DB_POOL_MIN: &str = "5";
pub(crate) const DEFAULT_DB_POOL_MAX: &str = "50";
pub(crate) const DEFAULT_DB_ACQUIRE_TIMEOUT_SECS: &str = "10";

pub(crate) const DEFAULT_MAX_POSITION_DURATION_SECS: &str = "172800";
pub(crate) const DEFAULT_PENDING_ORDER_TIMEOUT_SECS: &str = "60";
pub(crate) const DEFAULT_PRICE_POLL_INTERVAL_SECS: &str = "5";
pub(crate) const DEFAULT_CLOSE_ORDER_TIMEOUT_SECS: &str = "15";
pub(crate) const DEFAULT_HTTP_TIMEOUT_SECS: &str = "30";
pub(crate) const DEFAULT_LLM_TIMEOUT_SECS: &str = "120";

pub(crate) const DEFAULT_INITIAL_PRICE_MAX_RETRIES: &str = "10";
pub(crate) const DEFAULT_PERSIST_MAX_RETRIES: &str = "3";
pub(crate) const DEFAULT_PERSIST_RETRY_BASE_MS: &str = "100";

pub(crate) const DEFAULT_HTTP_CONNECT_TIMEOUT_SECS: &str = "10";
pub(crate) const DEFAULT_HTTP_POOL_MAX_IDLE_PER_HOST: &str = "10";

pub(crate) const DEFAULT_LISTENKEY_KEEPALIVE_FUTURES_SECS: &str = "1800";

pub(crate) fn parse_env_num<T: FromStr>(value: Option<String>, default: &str) -> VirsResult<T>
where
    <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    let s = value.unwrap_or_else(|| default.to_string());
    s.parse::<T>()
        .map_err(|e| VirsError::config(format!("Failed to parse '{}': {}", s, e)))
}

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

    pub acquire_timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminConfig {
    pub username: String,
    pub password: String,

    pub id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeConfig {
    pub max_position_duration_secs: u64,

    pub pending_order_timeout_secs: u64,

    pub price_poll_interval_secs: u64,

    pub close_order_timeout_secs: u64,

    pub http_timeout_secs: u64,

    pub llm_timeout_secs: u64,

    pub retry: RetryConfig,

    pub http: HttpConfig,

    pub listenkey: ListenKeyConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryConfig {
    pub initial_price_max_retries: u32,

    pub persist_max_retries: u32,

    pub persist_retry_base_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpConfig {
    pub http_connect_timeout_secs: u64,

    pub http_pool_max_idle_per_host: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListenKeyConfig {
    pub listenkey_keepalive_futures_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            initial_price_max_retries: DEFAULT_INITIAL_PRICE_MAX_RETRIES.parse().unwrap(),
            persist_max_retries: DEFAULT_PERSIST_MAX_RETRIES.parse().unwrap(),
            persist_retry_base_ms: DEFAULT_PERSIST_RETRY_BASE_MS.parse().unwrap(),
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            http_connect_timeout_secs: DEFAULT_HTTP_CONNECT_TIMEOUT_SECS.parse().unwrap(),
            http_pool_max_idle_per_host: DEFAULT_HTTP_POOL_MAX_IDLE_PER_HOST.parse().unwrap(),
        }
    }
}

impl Default for ListenKeyConfig {
    fn default() -> Self {
        Self {
            listenkey_keepalive_futures_secs: DEFAULT_LISTENKEY_KEEPALIVE_FUTURES_SECS
                .parse()
                .unwrap(),
        }
    }
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
            retry: RetryConfig::default(),
            http: HttpConfig::default(),
            listenkey: ListenKeyConfig::default(),
        }
    }
}

pub fn load_config() -> VirsResult<AppConfig> {
    dotenvy::dotenv().ok();
    load_config_from_env()
}

pub fn load_config_from_env() -> VirsResult<AppConfig> {
    let server = ServerConfig {
        host: std::env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.into()),
        port: parse_env_num(std::env::var("PORT").ok(), DEFAULT_PORT)?,
        encryption_key: std::env::var("ENCRYPTION_KEY")
            .map_err(|_| VirsError::config("ENCRYPTION_KEY environment variable is required"))?,
        llm_key: std::env::var("LLM_KEY")
            .map_err(|_| VirsError::config("LLM_KEY environment variable is required"))?,
        jwt_secret: std::env::var("JWT_SECRET")
            .map_err(|_| VirsError::config("JWT_SECRET environment variable is required"))?,
        jwt_expiration_hours: parse_env_num(
            std::env::var("JWT_EXPIRATION_HOURS").ok(),
            DEFAULT_JWT_HOURS,
        )?,
    };

    if server.encryption_key == server.llm_key {
        return Err(VirsError::config(
            "ENCRYPTION_KEY and LLM_KEY must be different — \
             sharing the same key breaks security isolation between exchange and LLM credential domains",
        ));
    }

    let database = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .map_err(|_| VirsError::config("DATABASE_URL environment variable is required"))?,
        pool_min: parse_env_num(std::env::var("DB_POOL_MIN").ok(), DEFAULT_DB_POOL_MIN)?,
        pool_max: parse_env_num(std::env::var("DB_POOL_MAX").ok(), DEFAULT_DB_POOL_MAX)?,
        acquire_timeout_secs: parse_env_num(
            std::env::var("DB_ACQUIRE_TIMEOUT_SECS").ok(),
            DEFAULT_DB_ACQUIRE_TIMEOUT_SECS,
        )?,
    };

    let admin = AdminConfig {
        username: std::env::var("ADMIN_USERNAME")
            .map_err(|_| VirsError::config("ADMIN_USERNAME environment variable is required"))?,
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
        retry: RetryConfig {
            initial_price_max_retries: parse_env_num(
                std::env::var("INITIAL_PRICE_MAX_RETRIES").ok(),
                DEFAULT_INITIAL_PRICE_MAX_RETRIES,
            )?,
            persist_max_retries: parse_env_num(
                std::env::var("PERSIST_MAX_RETRIES").ok(),
                DEFAULT_PERSIST_MAX_RETRIES,
            )?,
            persist_retry_base_ms: parse_env_num(
                std::env::var("PERSIST_RETRY_BASE_MS").ok(),
                DEFAULT_PERSIST_RETRY_BASE_MS,
            )?,
        },
        http: HttpConfig {
            http_connect_timeout_secs: parse_env_num(
                std::env::var("HTTP_CONNECT_TIMEOUT_SECS").ok(),
                DEFAULT_HTTP_CONNECT_TIMEOUT_SECS,
            )?,
            http_pool_max_idle_per_host: parse_env_num(
                std::env::var("HTTP_POOL_MAX_IDLE_PER_HOST").ok(),
                DEFAULT_HTTP_POOL_MAX_IDLE_PER_HOST,
            )?,
        },
        listenkey: ListenKeyConfig {
            listenkey_keepalive_futures_secs: parse_env_num(
                std::env::var("LISTENKEY_KEEPALIVE_FUTURES_SECS").ok(),
                DEFAULT_LISTENKEY_KEEPALIVE_FUTURES_SECS,
            )?,
        },
    };

    Ok(AppConfig {
        server,
        database,
        admin,
        time,
        proxy,
    })
}
