//! Exchange credentials handlers.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use tracing::info;

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

    let rows = sqlx::query_as::<_, (uuid::Uuid, String, String, String, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT id, exchange, label, market_type, created_at FROM qd_exchange_credentials WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await;

    match rows {
        Ok(creds) => Json(ApiResponse::ok(serde_json::json!({
            "items": creds.iter().map(|(id, exchange, label, market_type, created_at)| {
                serde_json::json!({
                    "id": id.to_string(),
                    "exchange": exchange,
                    "label": label,
                    "market_type": market_type,
                    "created_at": created_at.to_rfc3339(),
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

    let exchange = body["exchange"].as_str().unwrap_or("");
    let label = body["label"].as_str().unwrap_or("");
    let api_key = body["api_key"].as_str().unwrap_or("");
    let api_secret = body["api_secret"].as_str().unwrap_or("");
    let passphrase = body["passphrase"].as_str();
    let market_type = body["market_type"].as_str().unwrap_or("perpetual");

    if exchange.is_empty() || api_key.is_empty() || api_secret.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err("exchange, api_key, api_secret are required")),
        ));
    }

    let id = uuid::Uuid::new_v4();

    // Encrypt sensitive fields with AES-256-GCM
    let derived_key = virs_utils::crypto::derive_key(&state.encryption_key);
    let encrypted_api_key = virs_utils::crypto::encrypt(api_key, &derived_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Encryption error: {}", e)))))?;
    let encrypted_secret = virs_utils::crypto::encrypt(api_secret, &derived_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Encryption error: {}", e)))))?;
    let encrypted_passphrase = passphrase
        .map(|p| virs_utils::crypto::encrypt(p, &derived_key))
        .transpose()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Encryption error: {}", e)))))?;

    sqlx::query(
        r#"INSERT INTO qd_exchange_credentials (id, user_id, exchange, label, encrypted_api_key, encrypted_api_secret, encrypted_passphrase, market_type, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(exchange)
    .bind(label)
    .bind(&encrypted_api_key)
    .bind(&encrypted_secret)
    .bind(&encrypted_passphrase)
    .bind(market_type)
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
    })?;

    // Auto-register exchange in registry for immediate use
    let mt = match market_type {
        "spot" => virs_ccxt::MarketType::Spot,
        _ => virs_ccxt::MarketType::Perpetual,
    };
    if let Ok(ccxt_ex) = virs_ccxt::create_exchange(
        exchange, api_key, api_secret, passphrase, None, &mt,
    ) {
        let app_mt = match market_type {
            "spot" => virs_models::MarketType::Spot,
            _ => virs_models::MarketType::Perpetual,
        };
        let adapter = virs_exchange::CcxtAdapter::new(ccxt_ex, app_mt);
        state.exchange_registry.register(Box::new(adapter));
        info!(exchange, market_type, "Auto-registered exchange after credential save");
    }

    Ok(Json(ApiResponse::ok(serde_json::json!({"id": id.to_string()}))))
}

pub async fn delete_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let user_id = extract_user_id(&headers)?;

    sqlx::query(r#"DELETE FROM qd_exchange_credentials WHERE id = $1 AND user_id = $2"#)
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
    State(_state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse> {
    let exchange = body["exchange"].as_str().unwrap_or("");
    let api_key = body["api_key"].as_str().unwrap_or("");
    let api_secret = body["api_secret"].as_str().unwrap_or("");
    let passphrase = body["passphrase"].as_str();
    let market_type = body["market_type"].as_str().unwrap_or("perpetual");

    if exchange.is_empty() || api_key.is_empty() || api_secret.is_empty() {
        return Json(ApiResponse::err("exchange, api_key, api_secret are required"));
    }

    let mt = match market_type {
        "spot" => virs_ccxt::MarketType::Spot,
        _ => virs_ccxt::MarketType::Perpetual,
    };

    match virs_ccxt::create_exchange(exchange, api_key, api_secret, passphrase, None, &mt) {
        Ok(ccxt_ex) => {
            let app_mt = match market_type {
                "spot" => virs_models::MarketType::Spot,
                _ => virs_models::MarketType::Perpetual,
            };
            let adapter = virs_exchange::CcxtAdapter::new(ccxt_ex, app_mt);
            let exchange_ref: Box<dyn virs_exchange::Exchange> = Box::new(adapter);

            match exchange_ref.ping().await {
                Ok(true) => Json(ApiResponse::ok(serde_json::json!({
                    "connected": true,
                    "message": format!("Successfully connected to {}", exchange),
                }))),
                Ok(false) => Json(ApiResponse::ok(serde_json::json!({
                    "connected": false,
                    "message": format!("Ping returned false for {}", exchange),
                }))),
                Err(e) => Json(ApiResponse::ok(serde_json::json!({
                    "connected": false,
                    "message": format!("Connection test failed: {}", e),
                }))),
            }
        }
        Err(e) => Json(ApiResponse::ok(serde_json::json!({
            "connected": false,
            "message": format!("Failed to create exchange: {}", e),
        }))),
    }
}
