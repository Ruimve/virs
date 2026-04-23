use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    Json,
};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::api::AppState;
use crate::api::middleware::AuthUser;
use crate::models::{ApiResponse, LoginRequest, LoginResponse, UserResponse, User};

pub async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let ip = addr.ip().to_string();

    // Check failed login attempts from this IP in the last 15 minutes
    let fail_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM qd_login_attempts
           WHERE ip_address = $1 AND success = false
           AND attempt_time > NOW() - INTERVAL '15 minutes'"#,
    )
    .bind(&ip)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    if fail_count >= 5 {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiResponse::<serde_json::Value>::err("登录尝试过于频繁，请 15 分钟后再试")),
        ));
    }

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
            // Record failed attempt
            let _ = sqlx::query(
                r#"INSERT INTO qd_login_attempts (identifier, identifier_type, attempt_time, success, ip_address)
                   VALUES ($1, 'username', NOW(), false, $2)"#,
            )
            .bind(&req.username)
            .bind(&ip)
            .execute(&state.db_pool)
            .await;

            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<serde_json::Value>::err("Invalid credentials")),
            ));
        }
    };

    if !user.is_active {
        // Record failed attempt (account disabled)
        let _ = sqlx::query(
            r#"INSERT INTO qd_login_attempts (identifier, identifier_type, attempt_time, success, ip_address)
               VALUES ($1, 'username', NOW(), false, $2)"#,
        )
        .bind(&req.username)
        .bind(&ip)
        .execute(&state.db_pool)
        .await;

        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse::<serde_json::Value>::err("Account is disabled")),
        ));
    }

    let valid = bcrypt::verify(&req.password, &user.password_hash).unwrap_or(false);
    if !valid {
        // Record failed attempt (wrong password)
        let _ = sqlx::query(
            r#"INSERT INTO qd_login_attempts (identifier, identifier_type, attempt_time, success, ip_address)
               VALUES ($1, 'username', NOW(), false, $2)"#,
        )
        .bind(&req.username)
        .bind(&ip)
        .execute(&state.db_pool)
        .await;

        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<serde_json::Value>::err("Invalid credentials")),
        ));
    }

    // Record successful login attempt for audit
    let _ = sqlx::query(
        r#"INSERT INTO qd_login_attempts (identifier, identifier_type, attempt_time, success, ip_address)
           VALUES ($1, 'username', NOW(), true, $2)"#,
    )
    .bind(&req.username)
    .bind(&ip)
    .execute(&state.db_pool)
    .await;

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
