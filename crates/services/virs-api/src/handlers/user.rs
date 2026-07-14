use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use virs_error::VirsError;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

pub async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let users = sqlx::query_as::<_, (uuid::Uuid, String, String, Option<String>, bool, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT id, username, role, email, is_active, created_at FROM qd_users ORDER BY created_at DESC"#,
    )
    .fetch_all(&state.db_pool)
    .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "users": users.iter().map(|(id, username, role, email, is_active, created_at)| {
            serde_json::json!({
                "id": id.to_string(),
                "username": username,
                "role": role,
                "email": email,
                "is_active": is_active,
                "created_at": created_at.to_rfc3339(),
            })
        }).collect::<Vec<_>>()
    }))))
}

pub async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let username = body["username"].as_str().unwrap_or("");
    let password = body["password"].as_str().unwrap_or("");
    let role = body["role"].as_str().unwrap_or("user");
    let email = body["email"].as_str();

    if username.is_empty() || password.is_empty() {
        return Err(VirsError::bad_request(
            "Username and password are required",
        ));
    }

    let password_hash = virs_utils::crypto::hash_password(password)?;

    let id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO qd_users (id, username, password_hash, role, email, is_active, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, true, NOW(), NOW())"#,
    )
    .bind(id)
    .bind(username)
    .bind(&password_hash)
    .bind(role)
    .bind(email)
    .execute(&state.db_pool)
    .await?;

    Ok(Json(ApiResponse::ok(
        serde_json::json!({"id": id.to_string()}),
    )))
}

pub async fn update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let id = body["id"].as_str().unwrap_or("");
    let uuid_id = uuid::Uuid::parse_str(id)
        .map_err(|_| VirsError::bad_request("Invalid user ID"))?;

    let is_active = body["is_active"].as_bool();
    let role = body["role"].as_str();
    let email = body["email"].as_str();

    sqlx::query(
        r#"UPDATE qd_users SET role = COALESCE($2, role), email = COALESCE($3, email),
           is_active = COALESCE($4, is_active), updated_at = NOW() WHERE id = $1"#,
    )
    .bind(uuid_id)
    .bind(role)
    .bind(email)
    .bind(is_active)
    .execute(&state.db_pool)
    .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"updated": true}))))
}

pub async fn delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let id = body["id"].as_str().unwrap_or("");
    let uuid_id = uuid::Uuid::parse_str(id)
        .map_err(|_| VirsError::bad_request("Invalid user ID"))?;

    sqlx::query(r#"DELETE FROM qd_users WHERE id = $1"#)
        .bind(uuid_id)
        .execute(&state.db_pool)
        .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true}))))
}
