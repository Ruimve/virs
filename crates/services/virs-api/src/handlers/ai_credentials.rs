//! AI credentials handlers.

use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use virs_error::VirsError;

use crate::handlers::ai::{resolve_provider_base_url, resolve_provider_model};
use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

/// Parse the /models API response into a list of model objects.
/// Extracts id and owned_by from each model in the data array.
pub fn parse_models_response(data: &serde_json::Value) -> Vec<serde_json::Value> {
    data["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    m["id"].as_str().map(|id| serde_json::json!({
                        "id": id,
                        "owned_by": m["owned_by"].as_str().unwrap_or("unknown"),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

/// Parse the balance API response into a list of balance objects.
/// Checks balance_infos first, then falls back to data array.
pub fn parse_balance_response(data: &serde_json::Value) -> Vec<serde_json::Value> {
    data["balance_infos"]
        .as_array()
        .or_else(|| data["data"].as_array())
        .map(|arr| {
            arr.iter().map(|b| {
                let total_balance = b["total_balance"].as_str().unwrap_or_else(|| {
                    tracing::warn!("total_balance field missing in balance response — defaulting to '0'");
                    "0"
                });
                let currency = b["currency"].as_str().unwrap_or_else(|| {
                    tracing::warn!("currency field missing in balance response — defaulting to 'USD'");
                    "USD"
                });
                serde_json::json!({
                    "total_balance": total_balance,
                    "currency": currency,
                })
            }).collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub async fn list_credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let creds = sqlx::query_as::<_, (uuid::Uuid, String, Option<String>, bool, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT id, provider, label, is_default, created_at, updated_at FROM qd_ai_credentials WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
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
    }))))
}

pub async fn save_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let provider = body["provider"].as_str().unwrap_or("");
    let label = body["label"].as_str().unwrap_or("");
    let api_key = body["api_key"].as_str().unwrap_or("");
    let model = body["model"].as_str().unwrap_or("");
    let is_default = body["is_default"].as_bool().unwrap_or(false);

    if provider.is_empty() || api_key.is_empty() {
        return Err(VirsError::bad_request(
            "provider and api_key are required",
        ));
    }

    let id = uuid::Uuid::new_v4();

    // Encrypt API key with AES-256-GCM
    let encrypted_key =
        virs_utils::crypto::encrypt_with_key(api_key, &state.llm_key)?;

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
    .await?;

    Ok(Json(ApiResponse::ok(
        serde_json::json!({"id": id.to_string()}),
    )))
}

pub async fn delete_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    sqlx::query(r#"DELETE FROM qd_ai_credentials WHERE id = $1 AND user_id = $2"#)
        .bind(id)
        .bind(user_id)
        .execute(&state.db_pool)
        .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true}))))
}

/// GET /api/ai-credentials/test — test LLM connectivity using saved credentials.
pub async fn test_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    // Decrypt saved credential
    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials WHERE user_id = $1 AND is_default = true ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await?;

    let (provider, api_key) = match row {
        Some((p, enc_key)) => {
            let key =
                virs_utils::crypto::decrypt_with_key(&enc_key, &state.llm_key)?;
            (p, key)
        }
        None => {
            return Ok(Json(ApiResponse::ok(serde_json::json!({
                "connected": false,
                "message": "No AI credentials saved. Please save credentials first.",
            }))))
        }
    };

    let base_url = match resolve_provider_base_url(&provider) {
        Some(url) => url,
        None => {
            return Err(VirsError::bad_request(format!(
                "Unknown provider: {}",
                provider
            )))
        }
    };

    let model = resolve_provider_model(&provider).unwrap_or("deepseek-chat");

    let http_client = &state.http_client;
    match virs_bot::common::ai_client::call_llm_api(
        http_client,
        &api_key,
        base_url,
        model,
        "You are a test assistant. Always respond in json format.",
        "Return a json object with key \"status\" set to \"ok\".",
        &provider,
    )
    .await
    {
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
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials WHERE user_id = $1 AND is_default = true ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await?;

    let (provider, api_key) = match row {
        Some((p, enc_key)) => {
            let key =
                virs_utils::crypto::decrypt_with_key(&enc_key, &state.llm_key)?;
            (p, key)
        }
        None => {
            return Ok(Json(ApiResponse::ok(serde_json::json!({
                "models": [],
            }))))
        }
    };

    let base_url = match resolve_provider_base_url(&provider) {
        Some(url) => url,
        None => {
            return Err(VirsError::bad_request(format!(
                "Unknown provider: {}",
                provider
            )))
        }
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
                return Err(VirsError::bad_request(format!(
                    "Failed to fetch models: HTTP {}",
                    response.status()
                )));
            }
            match response.json::<serde_json::Value>().await {
                Ok(data) => {
                    let models = parse_models_response(&data);
                    Ok(Json(ApiResponse::ok(
                        serde_json::json!({ "models": models }),
                    )))
                }
                Err(e) => Err(VirsError::bad_request(format!(
                    "Failed to parse models: {}",
                    e
                ))),
            }
        }
        Err(e) => Err(VirsError::bad_request(format!(
            "Failed to fetch models: {}",
            e
        ))),
    }
}

/// GET /api/ai-credentials/balance — fetch account balance from LLM provider using saved credentials.
pub async fn fetch_balance(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials WHERE user_id = $1 AND is_default = true ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await?;

    let (provider, api_key) = match row {
        Some((p, enc_key)) => {
            let key =
                virs_utils::crypto::decrypt_with_key(&enc_key, &state.llm_key)?;
            (p, key)
        }
        None => {
            return Ok(Json(ApiResponse::ok(serde_json::json!({
                "balances": [],
            }))))
        }
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
                    let balances = parse_balance_response(&data);
                    Ok(Json(ApiResponse::ok(
                        serde_json::json!({ "balances": balances }),
                    )))
                }
                Err(_) => Ok(Json(ApiResponse::ok(serde_json::json!({ "balances": [] })))),
            }
        }
        Err(_) => Ok(Json(ApiResponse::ok(serde_json::json!({ "balances": [] })))),
    }
}
