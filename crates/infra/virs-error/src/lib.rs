pub mod api;
pub mod bot;
pub mod classify;
pub mod context;
pub mod exchange;

pub use api::ApiError;
pub use bot::{BotError, BotResult};
pub use classify::{Categorized, ErrorCategory, ErrorCode, HttpStatus, Retryable};
pub use context::Context;
pub use exchange::ExchangeError;

#[derive(Debug, thiserror::Error)]
pub enum VirsError {
    #[error(transparent)]
    Bot(#[from] BotError),

    #[error(transparent)]
    Exchange(#[from] ExchangeError),

    #[cfg(feature = "sqlx")]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("{message}")]
    Http { status: u16, message: String },

    #[error("Config error: {0}")]
    Config(String),

    #[error("Auth error: {0}")]
    Auth(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl VirsError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::Http {
            status: 400,
            message: msg.into(),
        }
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Http {
            status: 401,
            message: msg.into(),
        }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::Http {
            status: 404,
            message: msg.into(),
        }
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Http {
            status: 409,
            message: msg.into(),
        }
    }
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
    pub fn auth(msg: impl Into<String>) -> Self {
        Self::Auth(msg.into())
    }
    pub fn crypto(msg: impl Into<String>) -> Self {
        Self::Crypto(msg.into())
    }
}

pub type VirsResult<T> = std::result::Result<T, VirsError>;

impl Retryable for VirsError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Bot(e) => e.is_retryable(),
            Self::Exchange(e) => e.is_retryable(),
            #[cfg(feature = "sqlx")]
            Self::Database(_) => true,
            Self::Config(_)
            | Self::Auth(_)
            | Self::Crypto(_)
            | Self::Http { .. }
            | Self::Other(_) => false,
        }
    }
}

impl Categorized for VirsError {
    fn category(&self) -> ErrorCategory {
        match self {
            Self::Bot(e) => e.category(),
            Self::Exchange(e) => e.category(),
            #[cfg(feature = "sqlx")]
            Self::Database(_) => ErrorCategory::Database,
            Self::Http { status, .. } => match status {
                401 => ErrorCategory::Authentication,
                404 => ErrorCategory::NotFound,
                409 => ErrorCategory::Conflict,
                _ => ErrorCategory::Validation,
            },
            Self::Config(_) => ErrorCategory::Config,
            Self::Auth(_) => ErrorCategory::Authentication,
            Self::Crypto(_) => ErrorCategory::Internal,
            Self::Other(_) => ErrorCategory::Internal,
        }
    }
}

impl HttpStatus for VirsError {
    fn http_status(&self) -> u16 {
        match self {
            Self::Bot(e) => e.http_status(),
            Self::Exchange(e) => e.http_status(),
            #[cfg(feature = "sqlx")]
            Self::Database(_) => 503,
            Self::Http { status, .. } => *status,
            Self::Config(_) => 500,
            Self::Auth(_) => 401,
            Self::Crypto(_) => 500,
            Self::Other(_) => 500,
        }
    }
}

impl ErrorCode for VirsError {
    fn error_code(&self) -> &'static str {
        match self {
            Self::Bot(e) => e.error_code(),
            Self::Exchange(e) => e.error_code(),
            #[cfg(feature = "sqlx")]
            Self::Database(_) => "DATABASE_ERROR",
            Self::Http { status, .. } => match status {
                400 => "BAD_REQUEST",
                401 => "UNAUTHORIZED",
                404 => "NOT_FOUND",
                409 => "CONFLICT",
                _ => "HTTP_ERROR",
            },
            Self::Config(_) => "CONFIG_ERROR",
            Self::Auth(_) => "AUTH_ERROR",
            Self::Crypto(_) => "CRYPTO_ERROR",
            Self::Other(_) => "INTERNAL_ERROR",
        }
    }
}
