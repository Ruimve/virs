//! Bot-layer error type — migrated from virs-types.

use crate::classify::{ErrorCategory, Retryable, Categorized, HttpStatus, ErrorCode};

/// Bot-layer error type
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    #[error("Order execution failed: {0}")]
    OrderExecution(String),
    #[error("Credential error: {0}")]
    Credential(String),
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl BotError {
    pub fn order_execution(msg: impl Into<String>) -> Self {
        Self::OrderExecution(msg.into())
    }
    pub fn credential(msg: impl Into<String>) -> Self {
        Self::Credential(msg.into())
    }
    pub fn llm(msg: impl Into<String>) -> Self {
        Self::Llm(msg.into())
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

/// Bot-layer result type
pub type BotResult<T> = std::result::Result<T, BotError>;

impl Retryable for BotError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::OrderExecution(_) | Self::Llm(_) => true,
            Self::Credential(_) | Self::Internal(_) => false,
        }
    }
}

impl Categorized for BotError {
    fn category(&self) -> ErrorCategory {
        match self {
            Self::OrderExecution(_) => ErrorCategory::Internal,
            Self::Credential(_) => ErrorCategory::Authentication,
            Self::Llm(_) => ErrorCategory::Internal,
            Self::Internal(_) => ErrorCategory::Internal,
        }
    }
}

impl HttpStatus for BotError {
    fn http_status(&self) -> u16 {
        match self {
            Self::Credential(_) => 401,
            _ => 500,
        }
    }
}

impl ErrorCode for BotError {
    fn error_code(&self) -> &'static str {
        match self {
            Self::OrderExecution(_) => "BOT_ORDER_EXECUTION_FAILED",
            Self::Credential(_) => "BOT_CREDENTIAL_ERROR",
            Self::Llm(_) => "BOT_LLM_ERROR",
            Self::Internal(_) => "BOT_INTERNAL",
        }
    }
}
