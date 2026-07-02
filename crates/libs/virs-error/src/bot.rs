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
    /// LLM HTTP request transport error (network/timeout/decode failure from reqwest).
    /// Classified as retryable network error so transient failures can be retried.
    #[cfg(feature = "reqwest")]
    #[error("LLM request error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

impl BotError {
    pub fn llm(msg: impl Into<String>) -> Self {
        Self::Llm(msg.into())
    }
}

/// Bot-layer result type
pub type BotResult<T> = std::result::Result<T, BotError>;

impl Retryable for BotError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::OrderExecution(_) | Self::Llm(_) => true,
            #[cfg(feature = "reqwest")]
            Self::Reqwest(_) => true,
            Self::Credential(_) => false,
        }
    }
}

impl Categorized for BotError {
    fn category(&self) -> ErrorCategory {
        match self {
            Self::OrderExecution(_) => ErrorCategory::Internal,
            Self::Credential(_) => ErrorCategory::Authentication,
            Self::Llm(_) => ErrorCategory::Internal,
            #[cfg(feature = "reqwest")]
            Self::Reqwest(_) => ErrorCategory::Network,
        }
    }
}

impl HttpStatus for BotError {
    fn http_status(&self) -> u16 {
        match self {
            Self::Credential(_) => 401,
            #[cfg(feature = "reqwest")]
            Self::Reqwest(_) => 503,
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
            #[cfg(feature = "reqwest")]
            Self::Reqwest(_) => "BOT_LLM_NETWORK_ERROR",
        }
    }
}
