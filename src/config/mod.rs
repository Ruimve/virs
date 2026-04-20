use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Master application configuration, loaded from environment variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: Option<RedisConfig>,
    pub admin: AdminConfig,
    pub ai: AiConfig,
    pub notification: NotificationConfig,
    pub strategy: StrategyConfig,
    pub cache: CacheConfig,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub secret_key: String,
    pub encryption_key: String,
    pub jwt_expiration_hours: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub pool_min: u32,
    pub pool_max: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    pub username: String,
    pub password: String,
    /// UUID of the initial admin user, set at startup after creation/lookup.
    pub id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub openrouter_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub deepseek_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub telegram: Option<TelegramConfig>,
    pub email: Option<EmailConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub executor_workers: usize,
    pub pending_order_worker_enabled: bool,
    pub pending_order_poll_interval_secs: u64,
    pub auto_restore_strategies: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub ttl_ticker: u64,
    pub ttl_kline_1m: u64,
    pub ttl_kline_5m: u64,
    pub ttl_kline_1h: u64,
    pub ttl_kline_1d: u64,
}

/// Load configuration from environment variables.
/// Falls back to sensible defaults for optional fields.
pub fn load_config() -> Result<AppConfig, anyhow::Error> {
    dotenvy::dotenv().ok();

    let server = ServerConfig {
        host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
        port: std::env::var("PORT")
            .unwrap_or_else(|_| "8080".into())
            .parse()?,
        log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
        secret_key: std::env::var("SECRET_KEY")
            .map_err(|_| anyhow::anyhow!("SECRET_KEY environment variable is required"))?,
        encryption_key: std::env::var("ENCRYPTION_KEY")
            .map_err(|_| anyhow::anyhow!("ENCRYPTION_KEY environment variable is required (must differ from SECRET_KEY)"))?,
        jwt_expiration_hours: std::env::var("JWT_EXPIRATION_HOURS")
            .unwrap_or_else(|_| "24".into())
            .parse()?,
    };

    let database = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable is required"))?,
        pool_min: std::env::var("DB_POOL_MIN")
            .unwrap_or_else(|_| "5".into())
            .parse()?,
        pool_max: std::env::var("DB_POOL_MAX")
            .unwrap_or_else(|_| "50".into())
            .parse()?,
    };

    let redis = std::env::var("REDIS_URL").ok().map(|url| RedisConfig {
        url,
        password: std::env::var("REDIS_PASSWORD").ok(),
    });

    let admin = AdminConfig {
        username: std::env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".into()),
        password: std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin123".into()),
        id: None, // Will be set in main.rs after DB lookup/creation
    };

    let ai = AiConfig {
        openrouter_api_key: std::env::var("OPENROUTER_API_KEY").ok(),
        openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
        deepseek_api_key: std::env::var("DEEPSEEK_API_KEY").ok(),
    };

    let notification = NotificationConfig {
        telegram: std::env::var("TELEGRAM_BOT_TOKEN").ok()
            .zip(std::env::var("TELEGRAM_CHAT_ID").ok())
            .map(|(bot_token, chat_id)| TelegramConfig { bot_token, chat_id }),
        email: std::env::var("SMTP_HOST").ok()
            .zip(std::env::var("SMTP_USERNAME").ok())
            .zip(std::env::var("SMTP_PASSWORD").ok())
            .map(|((host, username), password)| EmailConfig {
                host,
                port: std::env::var("SMTP_PORT")
                    .unwrap_or_else(|_| "587".into())
                    .parse()
                    .unwrap_or(587),
                username,
                password,
                from: std::env::var("SMTP_FROM")
                    .unwrap_or_else(|_| "noreply@virs.com".into()),
            }),
    };

    let strategy = StrategyConfig {
        executor_workers: std::env::var("STRATEGY_EXECUTOR_WORKERS")
            .unwrap_or_else(|_| "4".into())
            .parse()?,
        pending_order_worker_enabled: std::env::var("PENDING_ORDER_WORKER_ENABLED")
            .unwrap_or_else(|_| "true".into())
            .parse()?,
        pending_order_poll_interval_secs: std::env::var("PENDING_ORDER_POLL_INTERVAL_SECS")
            .unwrap_or_else(|_| "5".into())
            .parse()?,
        auto_restore_strategies: std::env::var("AUTO_RESTORE_STRATEGIES")
            .unwrap_or_else(|_| "true".into())
            .parse()?,
    };

    let cache = CacheConfig {
        ttl_ticker: std::env::var("CACHE_TTL_TICKER")
            .unwrap_or_else(|_| "10".into())
            .parse()?,
        ttl_kline_1m: std::env::var("CACHE_TTL_KLINE_1M")
            .unwrap_or_else(|_| "60".into())
            .parse()?,
        ttl_kline_5m: std::env::var("CACHE_TTL_KLINE_5M")
            .unwrap_or_else(|_| "120".into())
            .parse()?,
        ttl_kline_1h: std::env::var("CACHE_TTL_KLINE_1H")
            .unwrap_or_else(|_| "300".into())
            .parse()?,
        ttl_kline_1d: std::env::var("CACHE_TTL_KLINE_1D")
            .unwrap_or_else(|_| "3600".into())
            .parse()?,
    };

    let proxy = std::env::var("PROXY_URL").ok();

    Ok(AppConfig {
        server,
        database,
        redis,
        admin,
        ai,
        notification,
        strategy,
        cache,
        proxy,
    })
}
