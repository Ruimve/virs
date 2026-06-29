use serde::{Deserialize, Serialize};
use std::str::FromStr;

// ============================================================
// Default value constants
// ============================================================

pub(crate) const DEFAULT_HOST: &str = "0.0.0.0";
pub(crate) const DEFAULT_PORT: &str = "8080";
pub(crate) const DEFAULT_LOG_LEVEL: &str = "info";
pub(crate) const DEFAULT_JWT_HOURS: &str = "24";
pub(crate) const DEFAULT_DB_POOL_MIN: &str = "5";
pub(crate) const DEFAULT_DB_POOL_MAX: &str = "50";
pub(crate) const DEFAULT_SMTP_PORT: &str = "587";
pub(crate) const DEFAULT_SMTP_FROM: &str = "noreply@virs.com";
pub(crate) const DEFAULT_CACHE_TTL_TICKER: &str = "10";
pub(crate) const DEFAULT_CACHE_TTL_KLINE_1M: &str = "60";
pub(crate) const DEFAULT_CACHE_TTL_KLINE_5M: &str = "120";
pub(crate) const DEFAULT_CACHE_TTL_KLINE_1H: &str = "300";
pub(crate) const DEFAULT_CACHE_TTL_KLINE_1D: &str = "3600";
pub(crate) const DEFAULT_ADMIN_USERNAME: &str = "admin";
pub(crate) const DEFAULT_ADMIN_PASSWORD: &str = "admin123";

// ============================================================
// Pure parsing functions (idempotent, no side effects)
// ============================================================

/// Parse a boolean-like string: "true" or "1" → true, everything else → false.
///
/// Used for environment variables that accept truthy string values.
pub(crate) fn parse_bool_str(v: &str) -> bool {
    v == "true" || v == "1"
}

/// Parse the `PAPER_TRADING` environment variable value.
///
/// - `Some("true")` / `Some("1")` → `Some(true)`
/// - `Some("false")` / `Some("0")` / `Some(other)` → `Some(false)`
/// - `None` → `Some(true)` (default to safe paper-trading mode)
pub(crate) fn parse_paper_value(v: Option<String>) -> Option<bool> {
    v.map(|s| parse_bool_str(&s)).or(Some(true))
}

/// Parse an optional environment variable string into a numeric type, using a default when absent.
///
/// Returns an error if the value is present but cannot be parsed.
pub(crate) fn parse_env_num<T: FromStr>(value: Option<String>, default: &str) -> Result<T, anyhow::Error>
where
    <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    let s = value.unwrap_or_else(|| default.to_string());
    s.parse::<T>().map_err(|e| anyhow::anyhow!("Failed to parse '{}': {}", s, e))
}

// ============================================================
// Pure config construction functions (idempotent)
// ============================================================

/// Build Redis configuration from optional URL and password.
///
/// Returns `None` if no URL is provided (Redis is optional).
pub(crate) fn build_redis_config(
    url: Option<String>,
    password: Option<String>,
) -> Option<RedisConfig> {
    url.map(|url| RedisConfig { url, password })
}

/// Build Telegram notification configuration.
///
/// Both `bot_token` and `chat_id` must be present; otherwise returns `None`.
pub(crate) fn build_telegram_config(
    bot_token: Option<String>,
    chat_id: Option<String>,
) -> Option<TelegramConfig> {
    bot_token.zip(chat_id).map(|(bot_token, chat_id)| TelegramConfig { bot_token, chat_id })
}

/// Build Email notification configuration.
///
/// Requires `host`, `username`, and `password` to all be present.
/// `port` defaults to 587, `from` defaults to "noreply@virs.com".
pub(crate) fn build_email_config(
    host: Option<String>,
    username: Option<String>,
    password: Option<String>,
    port: Option<String>,
    from: Option<String>,
) -> Option<EmailConfig> {
    host.zip(username).zip(password).map(|((host, username), password)| EmailConfig {
        host,
        port: port
            .and_then(|p| p.parse().ok())
            .unwrap_or_else(|| DEFAULT_SMTP_PORT.parse().unwrap()),
        username,
        password,
        from: from.unwrap_or_else(|| DEFAULT_SMTP_FROM.to_string()),
    })
}

// ============================================================
// Configuration structs
// ============================================================

/// Master application configuration, loaded from environment variables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: Option<RedisConfig>,
    pub admin: AdminConfig,
    pub ai: AiConfig,
    pub notification: NotificationConfig,
    pub cache: CacheConfig,
    pub proxy: Option<String>,
    /// Paper trading mode (true = simulated, false = real exchange)
    #[serde(default = "default_paper")]
    pub paper: Option<bool>,
}

fn default_paper() -> Option<bool> {
    Some(true)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
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
pub struct RedisConfig {
    pub url: String,
    pub password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminConfig {
    pub username: String,
    pub password: String,
    /// UUID of the initial admin user, set at startup after creation/lookup.
    pub id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiConfig {
    pub openrouter_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub deepseek_api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub telegram: Option<TelegramConfig>,
    pub email: Option<EmailConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmailConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheConfig {
    pub ttl_ticker: u64,
    pub ttl_kline_1m: u64,
    pub ttl_kline_5m: u64,
    pub ttl_kline_1h: u64,
    pub ttl_kline_1d: u64,
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
pub fn load_config() -> Result<AppConfig, anyhow::Error> {
    dotenvy::dotenv().ok();
    load_config_from_env()
}

/// Load configuration from environment variables without loading `.env` file.
///
/// This is the pure env-reading portion of [`load_config`], extracted for
/// testability. All defaults and validation logic are identical.
pub fn load_config_from_env() -> Result<AppConfig, anyhow::Error> {

    let server = ServerConfig {
        host: std::env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.into()),
        port: parse_env_num(std::env::var("PORT").ok(), DEFAULT_PORT)?,
        log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| DEFAULT_LOG_LEVEL.into()),
        secret_key: std::env::var("SECRET_KEY")
            .map_err(|_| anyhow::anyhow!("SECRET_KEY environment variable is required"))?,
        encryption_key: std::env::var("ENCRYPTION_KEY").map_err(|_| {
            anyhow::anyhow!(
                "ENCRYPTION_KEY environment variable is required (must differ from SECRET_KEY)"
            )
        })?,
        jwt_expiration_hours: parse_env_num(std::env::var("JWT_EXPIRATION_HOURS").ok(), DEFAULT_JWT_HOURS)?,
    };

    let database = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable is required"))?,
        pool_min: parse_env_num(std::env::var("DB_POOL_MIN").ok(), DEFAULT_DB_POOL_MIN)?,
        pool_max: parse_env_num(std::env::var("DB_POOL_MAX").ok(), DEFAULT_DB_POOL_MAX)?,
    };

    let redis = build_redis_config(
        std::env::var("REDIS_URL").ok(),
        std::env::var("REDIS_PASSWORD").ok(),
    );

    let admin = AdminConfig {
        username: std::env::var("ADMIN_USERNAME").unwrap_or_else(|_| DEFAULT_ADMIN_USERNAME.into()),
        password: std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| DEFAULT_ADMIN_PASSWORD.into()),
        id: None,
    };

    let ai = AiConfig {
        openrouter_api_key: std::env::var("OPENROUTER_API_KEY").ok(),
        openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
        deepseek_api_key: std::env::var("DEEPSEEK_API_KEY").ok(),
    };

    let notification = NotificationConfig {
        telegram: build_telegram_config(
            std::env::var("TELEGRAM_BOT_TOKEN").ok(),
            std::env::var("TELEGRAM_CHAT_ID").ok(),
        ),
        email: build_email_config(
            std::env::var("SMTP_HOST").ok(),
            std::env::var("SMTP_USERNAME").ok(),
            std::env::var("SMTP_PASSWORD").ok(),
            std::env::var("SMTP_PORT").ok(),
            std::env::var("SMTP_FROM").ok(),
        ),
    };

    let cache = CacheConfig {
        ttl_ticker: parse_env_num(std::env::var("CACHE_TTL_TICKER").ok(), DEFAULT_CACHE_TTL_TICKER)?,
        ttl_kline_1m: parse_env_num(std::env::var("CACHE_TTL_KLINE_1M").ok(), DEFAULT_CACHE_TTL_KLINE_1M)?,
        ttl_kline_5m: parse_env_num(std::env::var("CACHE_TTL_KLINE_5M").ok(), DEFAULT_CACHE_TTL_KLINE_5M)?,
        ttl_kline_1h: parse_env_num(std::env::var("CACHE_TTL_KLINE_1H").ok(), DEFAULT_CACHE_TTL_KLINE_1H)?,
        ttl_kline_1d: parse_env_num(std::env::var("CACHE_TTL_KLINE_1D").ok(), DEFAULT_CACHE_TTL_KLINE_1D)?,
    };

    let proxy = std::env::var("PROXY_URL").ok();

    let paper = parse_paper_value(std::env::var("PAPER_TRADING").ok());

    Ok(AppConfig {
        server,
        database,
        redis,
        admin,
        ai,
        notification,
        cache,
        proxy,
        paper,
    })
}
