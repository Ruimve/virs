//! CCXT-style unified exchange error types — migrated from virs-ccxt.

use crate::classify::{ErrorCategory, Retryable, Categorized, HttpStatus, ErrorCode};

/// Unified exchange error type, similar to CCXT's error hierarchy.
#[derive(Debug, thiserror::Error)]
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
    ExchangeError { code: String, message: String },

    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),

    #[error("Order not found: {0}")]
    OrderNotFound(String),

    #[error("Not supported: {0}")]
    NotSupported(String),

    #[error("No data available: {0}")]
    NoData(String),

    #[error("HTTP error {status}: {body}")]
    Http { status: u16, body: String },

    /// 下单请求返回 503 "Unknown error" — 执行状态未知（可能已成交）。
    /// 必须先通过 fetch_order 或 WebSocket 回报确认，不得盲目重试。
    #[error("Order status unknown (may have been filled): {0}")]
    OrderStatusUnknown(String),

    /// IP 被币安封禁（HTTP 418）— 收到 429 后继续访问导致。
    /// 封禁时间从 2 分钟到 3 天，频繁违反会累进延长。
    #[error("IP banned by exchange: {0}")]
    IpBanned(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ExchangeError {
    /// Create a "no data available" error — used when the exchange returns
    /// empty results and we must NOT return mock/fake data.
    pub fn no_data(context: String) -> Self {
        Self::NoData(context)
    }
}

#[cfg(feature = "reqwest")]
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

impl Retryable for ExchangeError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::RateLimited(_) => true,
            Self::Http { status, .. } => *status >= 500,
            // OrderStatusUnknown: 请求可能已成交，盲目重试会导致重复下单
            Self::OrderStatusUnknown(_) => false,
            // IpBanned: IP 被封禁，重试只会延长封禁时间
            Self::IpBanned(_) => false,
            _ => false,
        }
    }
}

impl Categorized for ExchangeError {
    fn category(&self) -> ErrorCategory {
        match self {
            Self::Network(_) | Self::Http { .. } => ErrorCategory::Network,
            Self::Authentication(_) => ErrorCategory::Authentication,
            Self::RateLimited(_) | Self::IpBanned(_) => ErrorCategory::RateLimited,
            Self::InvalidRequest(_) | Self::InsufficientFunds(_) => ErrorCategory::Validation,
            Self::OrderNotFound(_) | Self::NoData(_) => ErrorCategory::NotFound,
            Self::NotSupported(_) => ErrorCategory::Internal,
            Self::ExchangeError { .. } | Self::Internal(_) | Self::OrderStatusUnknown(_) => {
                ErrorCategory::Internal
            }
        }
    }
}

impl HttpStatus for ExchangeError {
    fn http_status(&self) -> u16 {
        match self {
            Self::Authentication(_) => 401,
            Self::RateLimited(_) => 429,
            Self::IpBanned(_) => 418,
            Self::InvalidRequest(_) | Self::InsufficientFunds(_) => 400,
            Self::OrderNotFound(_) | Self::NoData(_) => 404,
            Self::NotSupported(_) => 501,
            Self::Http { status, .. } => *status,
            Self::Network(_) => 503,
            Self::OrderStatusUnknown(_) => 503,
            Self::ExchangeError { .. } | Self::Internal(_) => 502,
        }
    }
}

impl ErrorCode for ExchangeError {
    fn error_code(&self) -> &'static str {
        match self {
            Self::Network(_) => "EXCHANGE_NETWORK_ERROR",
            Self::Authentication(_) => "EXCHANGE_AUTH_FAILED",
            Self::RateLimited(_) => "EXCHANGE_RATE_LIMITED",
            Self::InvalidRequest(_) => "EXCHANGE_INVALID_REQUEST",
            Self::ExchangeError { .. } => "EXCHANGE_ERROR",
            Self::InsufficientFunds(_) => "EXCHANGE_INSUFFICIENT_FUNDS",
            Self::OrderNotFound(_) => "EXCHANGE_ORDER_NOT_FOUND",
            Self::NotSupported(_) => "EXCHANGE_NOT_SUPPORTED",
            Self::NoData(_) => "EXCHANGE_NO_DATA",
            Self::Http { .. } => "EXCHANGE_HTTP_ERROR",
            Self::OrderStatusUnknown(_) => "EXCHANGE_ORDER_STATUS_UNKNOWN",
            Self::IpBanned(_) => "EXCHANGE_IP_BANNED",
            Self::Internal(_) => "EXCHANGE_INTERNAL",
        }
    }
}
