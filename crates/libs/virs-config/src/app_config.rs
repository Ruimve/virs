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
pub(crate) const DEFAULT_DB_ACQUIRE_TIMEOUT_SECS: &str = "10";
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

// Retry config default constants
pub(crate) const DEFAULT_INITIAL_PRICE_MAX_RETRIES: &str = "10";
pub(crate) const DEFAULT_PERSIST_MAX_RETRIES: &str = "3";
pub(crate) const DEFAULT_PERSIST_RETRY_BASE_MS: &str = "100";

// HTTP client default constants
pub(crate) const DEFAULT_HTTP_CONNECT_TIMEOUT_SECS: &str = "10";
pub(crate) const DEFAULT_HTTP_POOL_MAX_IDLE_PER_HOST: &str = "10";

// WebSocket default constants
pub(crate) const DEFAULT_WS_RECONNECT_INITIAL_DELAY_SECS: &str = "1";
pub(crate) const DEFAULT_WS_RECONNECT_MAX_DELAY_SECS: &str = "60";
pub(crate) const DEFAULT_WS_PING_INTERVAL_SECS: &str = "30";
pub(crate) const DEFAULT_WS_MAX_LIFETIME_SECS: &str = "82800"; // 23h

// listenKey keepalive default constants
pub(crate) const DEFAULT_LISTENKEY_KEEPALIVE_FUTURES_SECS: &str = "1800"; // 30min
pub(crate) const DEFAULT_LISTENKEY_KEEPALIVE_SPOT_SECS: &str = "900"; // 15min

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
    /// 数据库连接获取超时（秒）— 从池中获取连接的等待时间
    pub acquire_timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminConfig {
    pub username: String,
    pub password: String,
    /// UUID of the initial admin user, set at startup after creation/lookup.
    pub id: Option<uuid::Uuid>,
}

/// T12: Business time-related configuration (timeouts and intervals that affect trading logic).
///
/// All values are loaded from environment variables with safe defaults.
/// On initialization, `warn!` is logged to ensure observability.
///
/// Infrastructure-level config (HTTP client, WebSocket, listenKey, retry) is
/// delegated to sub-config structs for separation of concerns.
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
    /// 重试配置 — Worker 启动价格重试 + persist! 宏 DB 持久化重试
    pub retry: RetryConfig,
    /// HTTP 客户端基础设施配置 — TCP 连接超时 + 连接池
    pub http: HttpConfig,
    /// WebSocket 基础设施配置 — 重连 / 心跳 / 生命周期
    pub ws: WsConfig,
    /// 币安 listenKey 保活配置 — 合约 / 现货
    pub listenkey: ListenKeyConfig,
}

/// 重试行为配置 — 控制 Worker 启动和 DB 持久化的重试行为。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryConfig {
    /// 初始价格获取最大重试次数 — Worker 启动时获取初始价格的重试上限。默认 10
    pub initial_price_max_retries: u32,
    /// persist! 宏最大重试次数 — DB 持久化失败后的重试上限。默认 3
    pub persist_max_retries: u32,
    /// persist! 宏重试退避基数（毫秒）— 每次重试间隔 = base_ms × attempt。默认 100
    pub persist_retry_base_ms: u64,
}

/// HTTP 客户端基础设施配置 — 控制 reqwest 连接池行为。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpConfig {
    /// HTTP TCP 连接建立超时（秒）— 与 TimeConfig.http_timeout_secs（请求总超时）不同。默认 10
    pub http_connect_timeout_secs: u64,
    /// HTTP 连接池每主机最大空闲连接数。默认 10
    pub http_pool_max_idle_per_host: usize,
}

/// WebSocket 基础设施配置 — 控制 WS 连接的重连、心跳和生命周期。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WsConfig {
    /// WS 重连初始延迟（秒）。默认 1
    pub ws_reconnect_initial_delay_secs: u64,
    /// WS 重连最大延迟（秒）— 指数退避上限。默认 60
    pub ws_reconnect_max_delay_secs: u64,
    /// WS ping/pong 心跳间隔（秒）。默认 30
    pub ws_ping_interval_secs: u64,
    /// WS 连接最大生命周期（秒）— 到期后主动断开重连。默认 82800 (23h)
    pub ws_max_lifetime_secs: u64,
}

/// 币安 listenKey 保活配置 — 合约和现货的保活间隔。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListenKeyConfig {
    /// 合约 listenKey 保活间隔（秒）— 币安窗口 60min 的 1/2。默认 1800
    pub listenkey_keepalive_futures_secs: u64,
    /// 现货 listenKey 保活间隔（秒）— 币安窗口 30min 的 1/2。默认 900
    pub listenkey_keepalive_spot_secs: u64,
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

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            ws_reconnect_initial_delay_secs: DEFAULT_WS_RECONNECT_INITIAL_DELAY_SECS.parse().unwrap(),
            ws_reconnect_max_delay_secs: DEFAULT_WS_RECONNECT_MAX_DELAY_SECS.parse().unwrap(),
            ws_ping_interval_secs: DEFAULT_WS_PING_INTERVAL_SECS.parse().unwrap(),
            ws_max_lifetime_secs: DEFAULT_WS_MAX_LIFETIME_SECS.parse().unwrap(),
        }
    }
}

impl Default for ListenKeyConfig {
    fn default() -> Self {
        Self {
            listenkey_keepalive_futures_secs: DEFAULT_LISTENKEY_KEEPALIVE_FUTURES_SECS.parse().unwrap(),
            listenkey_keepalive_spot_secs: DEFAULT_LISTENKEY_KEEPALIVE_SPOT_SECS.parse().unwrap(),
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
            ws: WsConfig::default(),
            listenkey: ListenKeyConfig::default(),
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

    // Security: ENCRYPTION_KEY and LLM_KEY must be different.
    // ENCRYPTION_KEY encrypts exchange API credentials; LLM_KEY encrypts AI/LLM API credentials.
    // Sharing the same key would break security isolation between the two credential domains.
    if server.encryption_key == server.llm_key {
        return Err(VirsError::config(
            "ENCRYPTION_KEY and LLM_KEY must be different — \
             sharing the same key breaks security isolation between exchange and LLM credential domains",
        ));
    }

    let database = DatabaseConfig {
        url: std::env::var("DATABASE_URL").map_err(|_| {
            VirsError::config("DATABASE_URL environment variable is required")
        })?,
        pool_min: parse_env_num(std::env::var("DB_POOL_MIN").ok(), DEFAULT_DB_POOL_MIN)?,
        pool_max: parse_env_num(std::env::var("DB_POOL_MAX").ok(), DEFAULT_DB_POOL_MAX)?,
        acquire_timeout_secs: parse_env_num(
            std::env::var("DB_ACQUIRE_TIMEOUT_SECS").ok(),
            DEFAULT_DB_ACQUIRE_TIMEOUT_SECS,
        )?,
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
        ws: WsConfig {
            ws_reconnect_initial_delay_secs: parse_env_num(
                std::env::var("WS_RECONNECT_INITIAL_DELAY_SECS").ok(),
                DEFAULT_WS_RECONNECT_INITIAL_DELAY_SECS,
            )?,
            ws_reconnect_max_delay_secs: parse_env_num(
                std::env::var("WS_RECONNECT_MAX_DELAY_SECS").ok(),
                DEFAULT_WS_RECONNECT_MAX_DELAY_SECS,
            )?,
            ws_ping_interval_secs: parse_env_num(
                std::env::var("WS_PING_INTERVAL_SECS").ok(),
                DEFAULT_WS_PING_INTERVAL_SECS,
            )?,
            ws_max_lifetime_secs: parse_env_num(
                std::env::var("WS_MAX_LIFETIME_SECS").ok(),
                DEFAULT_WS_MAX_LIFETIME_SECS,
            )?,
        },
        listenkey: ListenKeyConfig {
            listenkey_keepalive_futures_secs: parse_env_num(
                std::env::var("LISTENKEY_KEEPALIVE_FUTURES_SECS").ok(),
                DEFAULT_LISTENKEY_KEEPALIVE_FUTURES_SECS,
            )?,
            listenkey_keepalive_spot_secs: parse_env_num(
                std::env::var("LISTENKEY_KEEPALIVE_SPOT_SECS").ok(),
                DEFAULT_LISTENKEY_KEEPALIVE_SPOT_SECS,
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
