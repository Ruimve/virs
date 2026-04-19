//! CCXT-style unified exchange error types.
//!
//! Provides a structured error hierarchy following CCXT's design philosophy:
//! - Network errors (timeout, connection refused)
//! - Authentication errors (invalid keys, expired signature)
//! - Rate limit errors (429, exchange-specific limits)
//! - Invalid request errors (bad parameters, unsupported pairs)
//! - Exchange errors (exchange-specific error codes)
//! - Insufficient funds
//! - Order not found
//! - Not supported (feature not implemented for this exchange)

use thiserror::Error;

/// Unified exchange error type, similar to CCXT's error hierarchy.
#[derive(Debug, Error)]
pub enum ExchangeError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Exchange error [{code}]: {message}")]
    ExchangeError {
        code: String,
        message: String,
    },

    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),

    #[error("Order not found: {0}")]
    OrderNotFound(String),

    #[error("Not supported: {0}")]
    NotSupported(String),

    #[error("No data available: {0}")]
    NoData(String),

    #[error("HTTP error {status}: {body}")]
    Http {
        status: u16,
        body: String,
    },

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ExchangeError {
    /// Create an exchange-specific error from a code and message.
    pub fn exchange(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ExchangeError {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Create a "no data available" error — used when the exchange returns
    /// empty results and we must NOT return mock/fake data.
    pub fn no_data(context: impl Into<String>) -> Self {
        Self::NoData(context.into())
    }

    /// Check if this error is retryable (network, rate limit, internal).
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ExchangeError::Network(_)
                | ExchangeError::RateLimited(_)
                | ExchangeError::Internal(_)
                | ExchangeError::Http { status: 429 | 500 | 502 | 503 | 504, .. }
        )
    }
}

impl From<reqwest::Error> for ExchangeError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() || err.is_connect() {
            ExchangeError::Network(err.to_string())
        } else if let Some(status) = err.status() {
            ExchangeError::Http {
                status: status.as_u16(),
                body: err.to_string(),
            }
        } else {
            ExchangeError::Network(err.to_string())
        }
    }
}
