use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use virs_error::VirsError;
use virs_database as db;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

pub async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let users = db::list_users(&state.db_pool).await?;

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

    let username = body["username"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("username is required"))?;
    let password = body["password"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("password is required"))?;
    let role = body["role"].as_str().unwrap_or("user");

    if role != "user" {
        return Err(VirsError::bad_request(
            "Invalid role — only 'user' role is allowed for self-registration",
        ));
    }
    let email = body["email"].as_str();

    let password_hash = virs_utils::hash_password(password)?;

    let id = uuid::Uuid::new_v4();
    db::create_user(
        &state.db_pool,
        id,
        username,
        &password_hash,
        role,
        email,
    )
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

    let id = body["id"]
        .as_str()
        .ok_or_else(|| VirsError::bad_request("id is required"))?;
    let uuid_id = uuid::Uuid::parse_str(id)
        .map_err(|_| VirsError::bad_request("Invalid user ID"))?;

    let is_active = body["is_active"].as_bool();
    let role = body["role"].as_str();
    let email = body["email"].as_str();

    db::update_user(
        &state.db_pool,
        uuid_id,
        role,
        email,
        is_active,
    )
    .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"updated": true}))))
}

pub async fn delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let id = body["id"]
        .as_str()
        .ok_or_else(|| VirsError::bad_request("id is required"))?;
    let uuid_id = uuid::Uuid::parse_str(id)
        .map_err(|_| VirsError::bad_request("Invalid user ID"))?;

    db::delete_user(&state.db_pool, uuid_id).await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true}))))
}
