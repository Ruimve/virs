use serde::Serialize;

use crate::classify::{Categorized, ErrorCode, HttpStatus, Retryable};
use crate::VirsError;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub success: bool,
    pub code: &'static str,
    pub category: String,
    pub retryable: bool,
    pub status: u16,
    pub message: String,
}

impl From<VirsError> for ApiError {
    fn from(e: VirsError) -> Self {
        let status = e.http_status();

        let message = if status >= 500 {
            "Internal server error".to_string()
        } else {
            e.to_string()
        };
        ApiError {
            success: false,
            code: e.error_code(),
            category: e.category().to_string(),
            retryable: e.is_retryable(),
            status,
            message,
        }
    }
}

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for VirsError {
    fn into_response(self) -> axum::response::Response {
        let status = axum::http::StatusCode::from_u16(self.http_status())
            .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        let body = axum::Json(ApiError::from(self));
        (status, body).into_response()
    }
}
