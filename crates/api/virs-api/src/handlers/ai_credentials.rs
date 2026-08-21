use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use virs_error::VirsError;
use virs_database as db;

use crate::handlers::ai::{resolve_provider_base_url, resolve_provider_model};
use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;


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


pub fn parse_balance_response(data: &serde_json::Value) -> Vec<serde_json::Value> {
    data["balance_infos"]
        .as_array()
        .or_else(|| data["data"].as_array())
        .map(|arr| {
            arr.iter().map(|b| {
                let total_balance = b["total_balance"].as_str();
                let currency = b["currency"].as_str();
                if total_balance.is_none() || currency.is_none() {
                    tracing::warn!(
                        total_balance = ?total_balance,
                        currency = ?currency,
                        "balance fields missing in response — returning null"
                    );
                }
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

    let creds = db::list_ai_credentials(&state.db_pool, user_id).await?;

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

    let provider = body["provider"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("provider is required"))?;
    let label = body["label"].as_str().unwrap_or("");
    let api_key = body["api_key"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("api_key is required"))?;
    let model = body["model"].as_str();
    let is_default = body["is_default"].as_bool().unwrap_or(false);

    let id = uuid::Uuid::new_v4();


    let encrypted_key =
        virs_utils::encrypt_with_key(api_key, &state.llm_key)?;

    db::save_ai_credential(
        &state.db_pool,
        id,
        user_id,
        provider,
        &encrypted_key,
        model,
        label,
        is_default,
    )
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

    db::delete_ai_credential(&state.db_pool, id, user_id).await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true}))))
}


pub async fn test_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;


    let row = db::get_default_ai_credential(&state.db_pool, user_id).await?;

    let (provider, api_key) = match row {
        Some((p, enc_key)) => {
            let key =
                virs_utils::decrypt_with_key(&enc_key, &state.llm_key)?;
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

    let model = resolve_provider_model(&provider)
        .expect("provider validated by resolve_provider_base_url above; get_provider_config shared");

    let http_client = &state.http_client;
    match virs_llm::call_llm_api(
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


pub async fn fetch_models(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let row = db::get_default_ai_credential(&state.db_pool, user_id).await?;

    let (provider, api_key) = match row {
        Some((p, enc_key)) => {
            let key =
                virs_utils::decrypt_with_key(&enc_key, &state.llm_key)?;
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


pub async fn fetch_balance(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let row = db::get_default_ai_credential(&state.db_pool, user_id).await?;

    let (provider, api_key) = match row {
        Some((p, enc_key)) => {
            let key =
                virs_utils::decrypt_with_key(&enc_key, &state.llm_key)?;
            (p, key)
        }
        None => {
            return Ok(Json(ApiResponse::ok(serde_json::json!({
                "balances": [],
            }))))
        }
    };

    let balance_url = match virs_type::LlmProviderConfig::for_provider(&provider).and_then(|c| c.balance_url) {
        Some(url) => url,
        None => return Ok(Json(ApiResponse::ok(serde_json::json!({ "balances": [] })))),
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
