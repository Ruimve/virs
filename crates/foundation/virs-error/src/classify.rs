use std::fmt;

use serde::{Deserialize, Serialize};

/* 错误分类枚举：对所有错误类型进行统一分类，用于 API 响应和日志分析 */
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

/* 可重试 trait：判断错误是否值得重试，用于自动重试逻辑的决策 */
pub trait Retryable {
    fn is_retryable(&self) -> bool;
}

/* 错误分类 trait：将错误映射为统一的 ErrorCategory */
pub trait Categorized {
    fn category(&self) -> ErrorCategory;
}

/* HTTP 状态码 trait：为错误提供对应的 HTTP 状态码 */
pub trait HttpStatus {
    fn http_status(&self) -> u16;
}

/* 错误码 trait：为错误提供稳定的字符串标识，用于前端和日志追踪 */
pub trait ErrorCode {
    fn error_code(&self) -> &'static str;
}
