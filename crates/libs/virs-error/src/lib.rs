//! VIRS unified error handling crate.
//!
//! All domain error types (BotError, ExchangeError) are defined here.
//! Upper layers use VirsError as the unified top-level error with
//! automatic `#[from]` conversion from each domain type.

pub mod api;
pub mod bot;
pub mod classify;
pub mod context;
pub mod exchange;

// Re-export all error types and result aliases at crate root
pub use api::ApiError;
pub use bot::{BotError, BotResult};
pub use classify::{Categorized, ErrorCategory, ErrorCode, HttpStatus, Retryable};
pub use context::Context;
pub use exchange::ExchangeError;

/// Top-level unified error.
///
/// Each domain error converts into VirsError via `#[from]`, so `?` works
/// seamlessly across crate boundaries. Callers can `match` on the domain
/// variant for fine-grained handling, or propagate upward to an
/// application boundary (HTTP handler, CLI main, etc.) where
/// `IntoResponse` / `is_retryable()` make the final decision.
#[derive(Debug, thiserror::Error)]
pub enum VirsError {
    #[error(transparent)]
    Bot(#[from] BotError),

    #[error(transparent)]
    Exchange(#[from] ExchangeError),

    /// Database error — classified as retryable, 503, category=database.
    #[cfg(feature = "sqlx")]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Generic HTTP-level error (validation, auth, not-found, conflict, etc.)
    /// used by API handlers for request-level failures that don't map to a
    /// specific domain error type.
    #[error("{message}")]
    Http { status: u16, message: String },

    /// Configuration loading or validation error.
    #[error("Config error: {0}")]
    Config(String),

    /// Authentication / JWT error.
    #[error("Auth error: {0}")]
    Auth(String),

    /// Cryptographic operation error (encrypt/decrypt/hash).
    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl VirsError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::Http { status: 400, message: msg.into() }
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Http { status: 401, message: msg.into() }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::Http { status: 404, message: msg.into() }
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Http { status: 409, message: msg.into() }
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

/// Unified result type for the entire VIRS platform.
pub type VirsResult<T> = std::result::Result<T, VirsError>;

// ---- Trait delegations on VirsError ----

impl Retryable for VirsError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Bot(e) => e.is_retryable(),
            Self::Exchange(e) => e.is_retryable(),
            #[cfg(feature = "sqlx")]
            Self::Database(_) => true,
            Self::Config(_) | Self::Auth(_) | Self::Crypto(_) | Self::Http { .. } | Self::Other(_) => false,
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
