//! AI credentials handlers.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::handlers::auth::{extract_user_id, ApiResponse};
use crate::state::AppState;

pub async fn list_credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<ApiResponse> {
    let user_id = match extract_user_id(&headers) {
        Ok(id) => id,
        Err((_, resp)) => return resp,
    };

    let rows = sqlx::query_as::<_, (uuid::Uuid, String, Option<String>, bool, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT id, provider, label, is_default, created_at, updated_at FROM qd_ai_credentials WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await;

    match rows {
        Ok(creds) => Json(ApiResponse::ok(serde_json::json!({
            "items": creds.iter().map(|(id, provider, label, is_default, created_at, updated_at)| {
                serde_json::json!({
                    "id": id.to_string(),
                    "provider": provider,
                    "label": label,
                    "is_default": is_default,
                    "created_at": created_at.to_rfc3339(),
                    "updated_at": updated_at.to_rfc3339(),
                })
            }).collect::<Vec<_>>()
        }))),
        Err(e) => Json(ApiResponse::err(format!("Database error: {}", e))),
    }
}

pub async fn save_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let user_id = extract_user_id(&headers)?;

    let provider = body["provider"].as_str().unwrap_or("");
    let label = body["label"].as_str().unwrap_or("");
    let api_key = body["api_key"].as_str().unwrap_or("");
    let is_default = body["is_default"].as_bool().unwrap_or(false);

    if provider.is_empty() || api_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err("provider and api_key are required")),
        ));
    }

    let id = uuid::Uuid::new_v4();

    // Encrypt API key with AES-256-GCM
    let derived_key = virs_utils::crypto::derive_key(&state.encryption_key);
    let encrypted_key = virs_utils::crypto::encrypt(api_key, &derived_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Encryption error: {}", e)))))?;

    sqlx::query(
        r#"INSERT INTO qd_ai_credentials (id, user_id, provider, encrypted_api_key, label, is_default, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, NOW())"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(provider)
    .bind(&encrypted_key)
    .bind(label)
    .bind(is_default)
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
    })?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"id": id.to_string()}))))
}

pub async fn delete_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let user_id = extract_user_id(&headers)?;

    sqlx::query(r#"DELETE FROM qd_ai_credentials WHERE id = $1 AND user_id = $2"#)
        .bind(id)
        .bind(user_id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
        })?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true}))))
}

pub async fn test_credential(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse> {
    let provider = body["provider"].as_str().unwrap_or("");
    let api_key = body["api_key"].as_str().unwrap_or("");
    let model = body["model"].as_str().unwrap_or("");
    let base_url = body["base_url"].as_str().unwrap_or("");

    if provider.is_empty() || api_key.is_empty() {
        return Json(ApiResponse::err("provider and api_key are required"));
    }

    let resolved_base_url = if base_url.is_empty() {
        match provider {
            "deepseek" => "https://api.deepseek.com",
            "openai" => "https://api.openai.com/v1",
            "openrouter" => "https://openrouter.ai/api/v1",
            _ => return Json(ApiResponse::err(format!("Unknown provider: {}", provider))),
        }
    } else {
        &base_url
    };

    let resolved_model = if model.is_empty() {
        match provider {
            "deepseek" => "deepseek-chat",
            "openai" => "gpt-4o",
            "openrouter" => "deepseek/deepseek-chat",
            _ => "deepseek-chat",
        }
    } else {
        &model
    };

    // Test by making a simple LLM API call
    let http_client = &state.http_client;
    match virs_bot::common::ai_client::call_llm_api(
        http_client,
        api_key,
        resolved_base_url,
        resolved_model,
        "You are a test assistant.",
        "Reply with: OK",
        provider,
    ).await {
        Ok(_) => Json(ApiResponse::ok(serde_json::json!({
            "connected": true,
            "message": format!("Successfully connected to {} ({})", provider, resolved_model),
        }))),
        Err(e) => Json(ApiResponse::ok(serde_json::json!({
            "connected": false,
            "message": format!("Connection test failed: {}", e),
        }))),
    }
}
