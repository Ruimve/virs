//! Position engine error type — migrated from virs-types.

use crate::classify::{ErrorCategory, Retryable, Categorized, HttpStatus, ErrorCode};
use crate::exchange::ExchangeError;

/// Position engine error
#[derive(Debug, thiserror::Error)]
pub enum PositionEngineError {
    #[error("Exchange: {0}")]
    Exchange(#[from] ExchangeError),
    #[error("Order not found: {order_id}")]
    OrderNotFound { order_id: String },
    #[error("Position not found: {position_id}")]
    PositionNotFound { position_id: String },
    #[error("Position already exists: {exchange}/{symbol}/{side}")]
    PositionAlreadyExists {
        exchange: String,
        symbol: String,
        side: String,
    },
    #[error("Invalid order amount: {amount}")]
    InvalidAmount { amount: f64 },
    #[error("Insufficient position size: requested={requested}, available={available}")]
    InsufficientPosition { requested: f64, available: f64 },
    #[cfg(feature = "sqlx")]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[cfg(not(feature = "sqlx"))]
    #[error("Database error: {0}")]
    Database(String),
    #[error("Engine not running")]
    EngineNotRunning,
    #[error("Engine already running")]
    EngineAlreadyRunning,
    #[error("Channel closed")]
    ChannelClosed,
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Position engine result type
pub type PositionResult<T> = std::result::Result<T, PositionEngineError>;

impl Retryable for PositionEngineError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Exchange(e) => e.is_retryable(),
            Self::Database(_) => true,
            Self::EngineNotRunning | Self::ChannelClosed => true,
            _ => false,
        }
    }
}

impl Categorized for PositionEngineError {
    fn category(&self) -> ErrorCategory {
        match self {
            Self::Exchange(e) => e.category(),
            Self::Database(_) => ErrorCategory::Database,
            Self::EngineNotRunning | Self::EngineAlreadyRunning | Self::ChannelClosed => {
                ErrorCategory::Internal
            }
            Self::Config(_) => ErrorCategory::Config,
            Self::OrderNotFound { .. } | Self::PositionNotFound { .. } => ErrorCategory::NotFound,
            Self::PositionAlreadyExists { .. } => ErrorCategory::Conflict,
            Self::InvalidAmount { .. } | Self::InsufficientPosition { .. } => {
                ErrorCategory::Validation
            }
        }
    }
}

impl HttpStatus for PositionEngineError {
    fn http_status(&self) -> u16 {
        match self {
            Self::Exchange(e) => e.http_status(),
            Self::Database(_) => 503,
            Self::OrderNotFound { .. } | Self::PositionNotFound { .. } => 404,
            Self::PositionAlreadyExists { .. } => 409,
            Self::InvalidAmount { .. }
            | Self::InsufficientPosition { .. } => 400,
            Self::Config(_) => 500,
            _ => 500,
        }
    }
}

impl ErrorCode for PositionEngineError {
    fn error_code(&self) -> &'static str {
        match self {
            Self::Exchange(e) => e.error_code(),
            Self::OrderNotFound { .. } => "PE_ORDER_NOT_FOUND",
            Self::PositionNotFound { .. } => "PE_POSITION_NOT_FOUND",
            Self::PositionAlreadyExists { .. } => "PE_POSITION_EXISTS",
            Self::InvalidAmount { .. } => "PE_INVALID_AMOUNT",
            Self::InsufficientPosition { .. } => "PE_INSUFFICIENT_POSITION",
            Self::Database(_) => "PE_DATABASE_ERROR",
            Self::EngineNotRunning => "PE_ENGINE_NOT_RUNNING",
            Self::EngineAlreadyRunning => "PE_ENGINE_ALREADY_RUNNING",
            Self::ChannelClosed => "PE_CHANNEL_CLOSED",
            Self::Config(_) => "PE_CONFIG_ERROR",
        }
    }
}
