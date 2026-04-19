use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::AppState;
use crate::api::middleware::RequireAdmin;
use crate::models::*;

enum UserUpdateParam {
    Text(String),
    Bool(bool),
    I64(i64),
}

#[derive(Deserialize)]
pub struct UserListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    _admin: RequireAdmin,
    Query(params): Query<UserListQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let (page, page_size) = (params.page.unwrap_or(1), params.page_size.unwrap_or(20));
    let offset = (page - 1) * page_size;

    let users = sqlx::query_as::<_, User>(
        r#"SELECT id, username, password_hash, role as "role", email, is_active, credits, created_at, updated_at
           FROM qd_users ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM qd_users")
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let user_responses: Vec<UserResponse> = users
        .into_iter()
        .map(|u| UserResponse {
            id: u.id,
            username: u.username,
            role: u.role,
            email: u.email,
            is_active: u.is_active,
            credits: u.credits,
            created_at: u.created_at,
        })
        .collect();

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": user_responses,
        "total": total,
        "page": page,
        "page_size": page_size,
    }))))
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    _admin: RequireAdmin,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<ApiResponse<UserResponse>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let password_hash = bcrypt::hash(&req.password, bcrypt::DEFAULT_COST).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Password hashing error: {}", e))),
        )
    })?;

    let role = req.role.unwrap_or(UserRole::User);
    let role_str = match role {
        UserRole::Admin => "admin",
        UserRole::Manager => "manager",
        UserRole::User => "user",
        UserRole::Viewer => "viewer",
    };

    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO qd_users (username, password_hash, role, email, is_active, credits)
           VALUES ($1, $2, $3, $4, true, 0)
           RETURNING id, username, password_hash, role as "role", email, is_active, credits, created_at, updated_at"#,
    )
    .bind(&req.username)
    .bind(&password_hash)
    .bind(role_str)
    .bind(&req.email)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Failed to create user: {}", e))),
        )
    })?;

    Ok(Json(ApiResponse::ok(UserResponse {
        id: user.id,
        username: user.username,
        role: user.role,
        email: user.email,
        is_active: user.is_active,
        credits: user.credits,
        created_at: user.created_at,
    })))
}

pub async fn update_user(
    State(state): State<Arc<AppState>>,
    _admin: RequireAdmin,
    Path(id): Path<Uuid>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let existing: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM qd_users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
            )
        })?;

    if existing.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("User not found")),
        ));
    }

    if let Some(password) = req.get("password").and_then(|v| v.as_str()) {
        if !password.is_empty() {
            let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<serde_json::Value>::err(format!("Password hashing error: {}", e))),
                )
            })?;
            sqlx::query("UPDATE qd_users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
                .bind(&hash)
                .bind(id)
                .execute(&state.db_pool)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiResponse::<serde_json::Value>::err(format!("Failed to update password: {}", e))),
                    )
                })?;
        }
    }

    let mut set_clauses: Vec<String> = Vec::new();
    let mut params: Vec<UserUpdateParam> = Vec::new();
    let mut bind_idx = 1;

    if let Some(value) = req.get("email").and_then(|v| v.as_str()) {
        set_clauses.push(format!("email = ${}", bind_idx));
        params.push(UserUpdateParam::Text(value.to_string()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("role").and_then(|v| v.as_str()) {
        set_clauses.push(format!("role = ${}", bind_idx));
        params.push(UserUpdateParam::Text(value.to_string()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("is_active").and_then(|v| v.as_bool()) {
        set_clauses.push(format!("is_active = ${}", bind_idx));
        params.push(UserUpdateParam::Bool(value));
        bind_idx += 1;
    }
    if let Some(value) = req.get("credits").and_then(|v| v.as_i64()) {
        set_clauses.push(format!("credits = ${}", bind_idx));
        params.push(UserUpdateParam::I64(value));
        bind_idx += 1;
    }

    if !set_clauses.is_empty() {
        set_clauses.push(format!("updated_at = NOW()"));
        let query_str = format!(
            "UPDATE qd_users SET {} WHERE id = ${}",
            set_clauses.join(", "),
            bind_idx
        );

        let mut query = sqlx::query(&query_str);
        for param in &params {
            query = match param {
                UserUpdateParam::Text(v) => query.bind(v),
                UserUpdateParam::Bool(v) => query.bind(*v),
                UserUpdateParam::I64(v) => query.bind(*v),
            };
        }
        query = query.bind(id);

        query.execute(&state.db_pool).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Failed to update user: {}", e))),
            )
        })?;
    }

    Ok(Json(ApiResponse::ok_with_message(serde_json::json!({"id": id}), "User updated")))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    _admin: RequireAdmin,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    if let Some(admin_id) = &state.config.admin.id {
        if id == *admin_id {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err("Cannot delete the initial admin user")),
            ));
        }
    }

    let result = sqlx::query("DELETE FROM qd_users WHERE id = $1")
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Delete failed: {}", e))),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("User not found")),
        ));
    }

    Ok(Json(ApiResponse::ok_with_message(serde_json::json!({"id": id}), "User deleted")))
}
