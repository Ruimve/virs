use thiserror::Error;

#[derive(Debug, Error)]
pub enum PositionEngineError {
    #[error("Exchange error: {0}")]
    Exchange(String),

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

    #[error("Risk check failed: {reason}")]
    RiskCheckFailed { reason: String },

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Engine not running")]
    EngineNotRunning,

    #[error("Engine already running")]
    EngineAlreadyRunning,

    #[error("Channel closed")]
    ChannelClosed,

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Position mode mismatch: expected={expected}, actual={actual}")]
    PositionModeMismatch { expected: String, actual: String },

    #[error("Position mode query failed: {0}")]
    PositionModeQueryFailed(String),
}

pub type Result<T> = std::result::Result<T, PositionEngineError>;
