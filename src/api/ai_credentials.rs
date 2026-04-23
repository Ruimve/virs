use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::middleware::AuthUser;
use crate::api::AppState;
use crate::models::*;
use crate::utils::crypto;

#[derive(Debug, Deserialize)]
pub struct SaveAiCredentialRequest {
    pub provider: String,
    pub api_key: String,
    pub label: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AiCredentialRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub label: Option<String>,
    pub is_default: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TestAiCredentialRequest {
    pub provider: String,
    pub api_key: String,
}

pub async fn list_credentials(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or(Uuid::nil());

    let rows = sqlx::query_as::<_, AiCredentialRow>(
        r#"SELECT id, user_id, provider, label, is_default, created_at, updated_at
           FROM qd_ai_credentials
           WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list AI credentials: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    Ok(Json(ApiResponse::ok(serde_json::json!({ "items": rows }))))
}

pub async fn save_credential(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<SaveAiCredentialRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or(Uuid::nil());

    // Validate provider
    let valid_providers = ["openrouter", "openai", "deepseek"];
    if !valid_providers.contains(&req.provider.to_lowercase().as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Invalid provider '{}'. Must be one of: openrouter, openai, deepseek",
                req.provider
            ))),
        ));
    }

    let provider = req.provider.to_lowercase();

    let encryption_key = crypto::derive_key(&state.config.server.encryption_key);
    let encrypted_key = crypto::encrypt(&req.api_key, &encryption_key).map_err(|e| {
        tracing::error!("Failed to encrypt AI credential: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Encryption error: {}", e))),
        )
    })?;

    let is_default = req.is_default.unwrap_or(false);

    let row = sqlx::query_as::<_, AiCredentialRow>(
        r#"INSERT INTO qd_ai_credentials
           (user_id, provider, encrypted_api_key, label, is_default)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (user_id, provider) DO UPDATE SET
           encrypted_api_key = $3, label = $4, is_default = $5, updated_at = NOW()
           RETURNING id, user_id, provider, label, is_default, created_at, updated_at"#,
    )
    .bind(user_id)
    .bind(&provider)
    .bind(&encrypted_key)
    .bind(&req.label)
    .bind(is_default)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to save AI credential: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to save AI credential: {}",
                e
            ))),
        )
    })?;

    tracing::info!(
        "AI credential saved for user {} provider {}",
        user_id,
        provider
    );

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "id": row.id,
        "provider": row.provider,
        "label": row.label,
        "is_default": row.is_default,
    }))))
}

pub async fn delete_credential(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or(Uuid::nil());

    let result = sqlx::query("DELETE FROM qd_ai_credentials WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete AI credential: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Delete failed: {}", e))),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("AI credential not found")),
        ));
    }

    tracing::info!("AI credential {} deleted by user {}", id, user_id);

    Ok(Json(ApiResponse::ok_with_message(serde_json::json!({"id": id}), "AI credential deleted")))
}

pub async fn test_credential(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(req): Json<TestAiCredentialRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let provider = req.provider.to_lowercase();

    let (base_url, model) = match provider.as_str() {
        "openrouter" => (
            "https://openrouter.ai/api/v1",
            "google/gemini-2.0-flash-001",
        ),
        "openai" => (
            "https://api.openai.com/v1",
            "gpt-4o-mini",
        ),
        "deepseek" => (
            "https://api.deepseek.com/v1",
            "deepseek-chat",
        ),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "Unknown AI provider: {}",
                    provider
                ))),
            ));
        }
    };

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "user", "content": "Hi" }
        ],
        "max_tokens": 5,
    });

    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", req.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("AI credential test request failed for {}: {}", provider, e);
            (
                StatusCode::BAD_GATEWAY,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "Connection test failed for {}: {}",
                    provider, e
                ))),
            )
        })?;

    if response.status().is_success() {
        tracing::info!("AI credential test succeeded for provider {}", provider);
        Ok(Json(ApiResponse::ok(serde_json::json!({
            "provider": provider,
            "valid": true,
            "message": format!("Successfully connected to {}", provider),
        }))))
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        tracing::warn!(
            "AI credential test failed for provider {}: {} - {}",
            provider,
            status,
            body
        );
        Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "API key test failed for {}: {} - {}",
                provider, status, body
            ))),
        ))
    }
}
