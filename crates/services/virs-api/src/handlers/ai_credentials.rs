//! AI credentials handlers.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::handlers::response::{extract_user_id, ApiResponse};
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
    let model = body["model"].as_str().unwrap_or("");
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
        r#"INSERT INTO qd_ai_credentials (id, user_id, provider, encrypted_api_key, model, label, is_default, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
           ON CONFLICT (user_id, provider)
           DO UPDATE SET encrypted_api_key = $4, model = $5, label = $6, is_default = $7, updated_at = NOW()"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(provider)
    .bind(&encrypted_key)
    .bind(if model.is_empty() { None as Option<&str> } else { Some(model) })
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

/// GET /api/ai-credentials/test — test LLM connectivity using saved credentials.
pub async fn test_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let user_id = extract_user_id(&headers)?;

    // Decrypt saved credential
    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials WHERE user_id = $1 AND is_default = true ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    let (provider, api_key) = match row {
        Some((p, enc_key)) => {
            let derived_key = virs_utils::crypto::derive_key(&state.encryption_key);
            let key = virs_utils::crypto::decrypt(&enc_key, &derived_key)
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err("Failed to decrypt API key"))))?;
            (p, key)
        }
        None => return Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": false,
            "message": "No AI credentials saved. Please save credentials first.",
        })))),
    };

    let base_url = match provider.as_str() {
        "deepseek" => "https://api.deepseek.com",
        "openai" => "https://api.openai.com/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        _ => return Ok(Json(ApiResponse::err(format!("Unknown provider: {}", provider)))),
    };

    let model = match provider.as_str() {
        "deepseek" => "deepseek-chat",
        "openai" => "gpt-4o",
        "openrouter" => "deepseek/deepseek-chat",
        _ => "deepseek-chat",
    };

    let http_client = &state.http_client;
    match virs_bot::common::ai_client::call_llm_api(
        http_client,
        &api_key,
        base_url,
        model,
        "You are a test assistant. Always respond in json format.",
        "Return a json object with key \"status\" set to \"ok\".",
        &provider,
    ).await {
        Ok(_) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": true,
            "message": format!("Successfully connected to {} ({})", provider, model),
        })))),
        Err(e) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": false,
            "message": format!("Connection test failed: {}", e),
        })))),
    }
}

/// GET /api/ai-credentials/models — fetch available models from LLM provider using saved credentials.
pub async fn fetch_models(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let user_id = extract_user_id(&headers)?;

    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials WHERE user_id = $1 AND is_default = true ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    let (provider, api_key) = match row {
        Some((p, enc_key)) => {
            let derived_key = virs_utils::crypto::derive_key(&state.encryption_key);
            let key = virs_utils::crypto::decrypt(&enc_key, &derived_key)
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err("Failed to decrypt API key"))))?;
            (p, key)
        }
        None => return Ok(Json(ApiResponse::ok(serde_json::json!({
            "models": [],
        })))),
    };

    let base_url = match provider.as_str() {
        "deepseek" => "https://api.deepseek.com",
        "openai" => "https://api.openai.com/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        _ => return Ok(Json(ApiResponse::err(format!("Unknown provider: {}", provider)))),
    };

    let models_url = format!("{}/models", base_url);
    let http_client = &state.http_client;
    let resp = http_client
        .get(&models_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await;

    match resp {
        Ok(response) => {
            if !response.status().is_success() {
                return Ok(Json(ApiResponse::err(format!("Failed to fetch models: HTTP {}", response.status()))));
            }
            match response.json::<serde_json::Value>().await {
                Ok(data) => {
                    let models = data["data"].as_array()
                        .map(|arr| arr.iter().filter_map(|m| {
                            m["id"].as_str().map(|id| serde_json::json!({
                                "id": id,
                                "owned_by": m["owned_by"].as_str().unwrap_or("unknown"),
                            }))
                        }).collect::<Vec<_>>())
                        .unwrap_or_default();
                    Ok(Json(ApiResponse::ok(serde_json::json!({ "models": models }))))
                }
                Err(e) => Ok(Json(ApiResponse::err(format!("Failed to parse models: {}", e)))),
            }
        }
        Err(e) => Ok(Json(ApiResponse::err(format!("Failed to fetch models: {}", e)))),
    }
}

/// GET /api/ai-credentials/balance — fetch account balance from LLM provider using saved credentials.
pub async fn fetch_balance(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let user_id = extract_user_id(&headers)?;

    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials WHERE user_id = $1 AND is_default = true ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    let (provider, api_key) = match row {
        Some((p, enc_key)) => {
            let derived_key = virs_utils::crypto::derive_key(&state.encryption_key);
            let key = virs_utils::crypto::decrypt(&enc_key, &derived_key)
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err("Failed to decrypt API key"))))?;
            (p, key)
        }
        None => return Ok(Json(ApiResponse::ok(serde_json::json!({
            "balances": [],
        })))),
    };

    let balance_url = match provider.as_str() {
        "deepseek" => "https://api.deepseek.com/user/balance",
        _ => return Ok(Json(ApiResponse::ok(serde_json::json!({ "balances": [] })))),
    };

    let http_client = &state.http_client;
    let resp = http_client
        .get(balance_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await;

    match resp {
        Ok(response) => {
            if !response.status().is_success() {
                return Ok(Json(ApiResponse::ok(serde_json::json!({ "balances": [] }))));
            }
            match response.json::<serde_json::Value>().await {
                Ok(data) => {
                    let balances = data["balance_infos"].as_array()
                        .or_else(|| data["data"].as_array())
                        .map(|arr| arr.iter().map(|b| serde_json::json!({
                            "total_balance": b["total_balance"].as_str().unwrap_or("0"),
                            "currency": b["currency"].as_str().unwrap_or("USD"),
                        })).collect::<Vec<_>>())
                        .unwrap_or_default();
                    Ok(Json(ApiResponse::ok(serde_json::json!({ "balances": balances }))))
                }
                Err(_) => Ok(Json(ApiResponse::ok(serde_json::json!({ "balances": [] })))),
            }
        }
        Err(_) => Ok(Json(ApiResponse::ok(serde_json::json!({ "balances": [] })))),
    }
}
