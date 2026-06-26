//! Common API response types.

use axum::{http::StatusCode, Json};

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

/// Convenience type for handler return values that can fail.
pub type ApiResult = Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)>;

/// Extract user_id from JWT in Authorization header.
/// Shared by all handlers that need user identity.
pub fn extract_user_id(
    headers: &axum::http::HeaderMap,
) -> Result<uuid::Uuid, (StatusCode, Json<ApiResponse>)> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::err("Missing or invalid authorization header")),
            ))
        }
    };

    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "virs-secret-key".to_string());
    let decoded = jsonwebtoken::decode::<serde_json::Value>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    );

    match decoded {
        Ok(data) => {
            let user_id = data.claims["sub"].as_str().unwrap_or("");
            uuid::Uuid::parse_str(user_id).map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(ApiResponse::err("Invalid user ID in token")),
                )
            })
        }
        Err(_) => Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::err("Invalid token")),
        )),
    }
}
