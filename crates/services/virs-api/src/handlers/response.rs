//! Common API response types.

use virs_error::VirsError;

/// API response wrapper — always uses serde_json::Value for data.
#[derive(serde::Serialize)]
pub struct ApiResponse {
    pub success: bool,
    pub data: serde_json::Value,
    pub message: Option<String>,
}

impl ApiResponse {
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: serde_json::Value::Null,
            message: Some(msg.into()),
        }
    }
}

/// Extract user_id from JWT in Authorization header.
/// Shared by all handlers that need user identity.
pub fn extract_user_id(headers: &axum::http::HeaderMap) -> Result<uuid::Uuid, VirsError> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(t) => t,
        None => {
            return Err(VirsError::unauthorized(
                "Missing or invalid authorization header",
            ))
        }
    };

    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "virs-secret-key".to_string());
    match virs_utils::auth::decode_jwt(token, &secret) {
        Ok(claims) => uuid::Uuid::parse_str(&claims.sub)
            .map_err(|_| VirsError::unauthorized("Invalid user ID in token")),
        Err(_) => Err(VirsError::unauthorized("Invalid token")),
    }
}
