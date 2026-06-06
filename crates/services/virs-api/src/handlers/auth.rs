//! Auth handlers — login, logout, user info.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::state::AppState;

/// Extract user_id from JWT in Authorization header.
/// Shared by all handlers that need user identity.
pub fn extract_user_id(headers: &HeaderMap) -> Result<uuid::Uuid, (StatusCode, Json<ApiResponse>)> {
    let auth_header = headers.get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(t) => t,
        None => return Err((StatusCode::UNAUTHORIZED, Json(ApiResponse::err("Missing or invalid authorization header")))),
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
            uuid::Uuid::parse_str(user_id)
                .map_err(|_| (StatusCode::UNAUTHORIZED, Json(ApiResponse::err("Invalid user ID in token"))))
        }
        Err(_) => Err((StatusCode::UNAUTHORIZED, Json(ApiResponse::err("Invalid token")))),
    }
}

/// Login request
#[derive(serde::Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response
#[derive(serde::Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

/// User info in response
#[derive(serde::Serialize)]
pub struct UserInfo {
    pub id: uuid::Uuid,
    pub username: String,
    pub role: String,
    pub email: Option<String>,
    pub is_active: bool,
}

/// API response wrapper — always uses serde_json::Value for data
#[derive(serde::Serialize)]
pub struct ApiResponse {
    pub success: bool,
    pub data: serde_json::Value,
    pub message: Option<String>,
}

impl ApiResponse {
    pub fn ok(data: serde_json::Value) -> Self {
        Self { success: true, data, message: None }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self { success: false, data: serde_json::Value::Null, message: Some(msg.into()) }
    }
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    // Query user from database
    let row = sqlx::query_as::<_, (uuid::Uuid, String, String, String, Option<String>, bool)>(
        r#"SELECT id, username, password_hash, role, email, is_active FROM qd_users WHERE username = $1"#,
    )
    .bind(&req.username)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(format!("Database error: {}", e))),
        )
    })?;

    let (id, username, password_hash, role, email, is_active) = match row {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::err("Invalid credentials")),
            ));
        }
    };

    if !is_active {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse::err("Account is disabled")),
        ));
    }

    let valid = bcrypt::verify(&req.password, &password_hash).unwrap_or(false);
    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::err("Invalid credentials")),
        ));
    }

    // Generate JWT token
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "virs-secret-key".to_string());
    let expiration_hours: i64 = std::env::var("JWT_EXPIRATION_HOURS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);

    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": id.to_string(),
        "username": username,
        "role": role,
        "exp": now + expiration_hours * 3600,
        "iat": now,
    });

    let header = jsonwebtoken::Header::default();
    let token = jsonwebtoken::encode(&header, &claims, &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::err(format!("JWT error: {}", e))),
            )
        })?;

    Ok(Json(ApiResponse::ok(serde_json::to_value(LoginResponse {
        token,
        user: UserInfo { id, username, role, email, is_active },
    }).unwrap_or_default())))
}

pub async fn logout() -> Json<ApiResponse> {
    Json(ApiResponse::ok(serde_json::json!({"message": "Logged out"})))
}

pub async fn get_user_info(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse> {
    // Extract token from Authorization header
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if token.is_empty() {
        return Json(ApiResponse::err("No token provided"));
    }

    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "virs-secret-key".to_string());
    let decoded = jsonwebtoken::decode::<serde_json::Value>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    );

    match decoded {
        Ok(data) => {
            let claims = data.claims;
            let user_id = claims["sub"].as_str().unwrap_or("");
            let _username = claims["username"].as_str().unwrap_or("");
            let _role = claims["role"].as_str().unwrap_or("");

            if user_id.is_empty() {
                return Json(ApiResponse::err("Invalid token"));
            }

            // Verify user still exists and is active
            let uuid_id = match uuid::Uuid::parse_str(user_id) {
                Ok(id) => id,
                Err(_) => return Json(ApiResponse::err("Invalid user ID in token")),
            };

            let row = sqlx::query_as::<_, (String, String, Option<String>, bool)>(
                r#"SELECT username, role, email, is_active FROM qd_users WHERE id = $1"#,
            )
            .bind(uuid_id)
            .fetch_optional(&state.db_pool)
            .await;

            match row {
                Ok(Some((db_username, db_role, email, is_active))) => {
                    if !is_active {
                        return Json(ApiResponse::err("Account is disabled"));
                    }
                    Json(ApiResponse::ok(serde_json::json!({
                        "id": user_id,
                        "username": db_username,
                        "role": db_role,
                        "email": email,
                        "is_active": is_active,
                    })))
                }
                Ok(None) => Json(ApiResponse::err("User not found")),
                Err(e) => Json(ApiResponse::err(format!("Database error: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("Invalid token: {}", e))),
    }
}
