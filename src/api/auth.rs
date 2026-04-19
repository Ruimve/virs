use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::api::AppState;
use crate::api::middleware::AuthUser;
use crate::models::{ApiResponse, LoginRequest, LoginResponse, UserResponse, User};

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, username, password_hash, role as "role", email, is_active, credits, created_at, updated_at FROM qd_users WHERE username = $1"#,
    )
    .bind(&req.username)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let user = match user {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<serde_json::Value>::err("Invalid credentials")),
            ));
        }
    };

    if !user.is_active {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse::<serde_json::Value>::err("Account is disabled")),
        ));
    }

    let valid = bcrypt::verify(&req.password, &user.password_hash).unwrap_or(false);
    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<serde_json::Value>::err("Invalid credentials")),
        ));
    }

    let now = chrono::Utc::now().timestamp();
    let claims = crate::utils::auth::Claims {
        sub: user.id.to_string(),
        username: user.username.clone(),
        role: format!("{:?}", user.role),
        exp: now + state.config.server.jwt_expiration_hours * 3600,
        iat: now,
    };

    let token = crate::utils::auth::encode_jwt(&claims, &state.config.server.secret_key)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("JWT error: {}", e))),
            )
        })?;

    Ok(Json(ApiResponse::ok(LoginResponse {
        token,
        user: UserResponse {
            id: user.id,
            username: user.username,
            role: user.role,
            email: user.email,
            is_active: user.is_active,
            credits: user.credits,
            created_at: user.created_at,
        },
    })))
}

pub async fn get_user_info(
    auth: AuthUser,
) -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::ok(serde_json::json!({
        "id": auth.user_id,
        "username": auth.username,
        "role": format!("{:?}", auth.role),
    })))
}

pub async fn logout() -> Json<ApiResponse<()>> {
    Json(ApiResponse::ok_with_message((), "Logged out successfully. Please discard your JWT token."))
}
