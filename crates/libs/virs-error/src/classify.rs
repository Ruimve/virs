//! Error classification traits and ErrorCategory enum.
//!
//! Every domain error type implements these traits so that upper layers
//! (HTTP handlers, retry loops, alerting) can make uniform decisions
//! without knowing the concrete error variant.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Broad error category for aggregation / alerting / HTTP mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Network,
    RateLimited,
    Authentication,
    Validation,
    NotFound,
    Conflict,
    Database,
    Config,
    Internal,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network => write!(f, "network"),
            Self::RateLimited => write!(f, "rate_limited"),
            Self::Authentication => write!(f, "authentication"),
            Self::Validation => write!(f, "validation"),
            Self::NotFound => write!(f, "not_found"),
            Self::Conflict => write!(f, "conflict"),
            Self::Database => write!(f, "database"),
            Self::Config => write!(f, "config"),
            Self::Internal => write!(f, "internal"),
        }
    }
}

/// Whether the operation can be retried (transient failure).
pub trait Retryable {
    fn is_retryable(&self) -> bool;
}

/// Categorical classification for alerting / metrics.
pub trait Categorized {
    fn category(&self) -> ErrorCategory;
}

/// HTTP status code mapping (used by `IntoResponse`).
pub trait HttpStatus {
    fn http_status(&self) -> u16;
}

/// Stable error code for frontend / API consumers.
pub trait ErrorCode {
    fn error_code(&self) -> &'static str;
}
