//! Exchange credentials handlers.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use tracing::info;

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
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
           ON CONFLICT (user_id, exchange, market_type)
           DO UPDATE SET encrypted_api_key = $5, encrypted_api_secret = $6, encrypted_passphrase = $7, label = $4, updated_at = NOW()"#,
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

/// POST /api/credentials/test — test connectivity only (ping).
/// Uses the exchange already registered in the registry (saved by save_credential).
pub async fn test_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let _user_id = extract_user_id(&headers)?;

    let names = state.exchange_registry.registered_names();
    let exchange_ref = names.first().and_then(|key| state.exchange_registry.get(key));

    let exchange = match exchange_ref {
        Some(e) => e,
        None => return Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": false,
            "message": "No exchange registered. Please save credentials first.",
        })))),
    };

    match exchange.ping().await {
        Ok(true) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": true,
        })))),
        Ok(false) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": false,
            "message": "Ping returned false",
        })))),
        Err(e) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": false,
            "message": format!("{}", e),
        })))),
    }
}

/// POST /api/credentials/check-permissions — check API key permissions via /sapi/v1/account/apiRestrictions.
/// Uses the exchange already registered in the registry (saved by save_credential).
pub async fn check_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let _user_id = extract_user_id(&headers)?;

    let names = state.exchange_registry.registered_names();
    let exchange_ref = names.first().and_then(|key| state.exchange_registry.get(key));

    let exchange = match exchange_ref {
        Some(e) => e,
        None => return Ok(Json(ApiResponse::ok(serde_json::json!({
            "permissions": [{
                "name": "connectivity",
                "label": "Connectivity",
                "status": "error",
                "detail": "No exchange registered. Please save credentials first.",
            }],
        })))),
    };

    // Call apiRestrictions
    match exchange.get_api_restrictions().await {
        Ok(restrictions) => {
            let mut permissions = Vec::new();

            // Note: "connectivity" is NOT included here — it's already checked
            // by the separate /test (ping) endpoint in Step 2.

            permissions.push(serde_json::json!({
                "name": "read_info",
                "label": "Read Info",
                "status": if restrictions.read_info { "ok" } else { "error" },
                "detail": if restrictions.read_info { "Reading account info enabled" } else { "Reading account info disabled" }
            }));

            permissions.push(serde_json::json!({
                "name": "spot_trading",
                "label": "Spot & Margin Trading",
                "status": if restrictions.enable_spot_and_margin_trading { "ok" } else { "error" },
                "detail": if restrictions.enable_spot_and_margin_trading { "Spot and margin trading enabled" } else { "Spot and margin trading disabled" }
            }));

            permissions.push(serde_json::json!({
                "name": "futures",
                "label": "Futures Trading",
                "status": if restrictions.enable_futures { "ok" } else { "error" },
                "detail": if restrictions.enable_futures { "Futures trading enabled" } else { "Futures trading disabled" }
            }));

            permissions.push(serde_json::json!({
                "name": "withdrawals",
                "label": "Withdrawals",
                "status": if restrictions.enable_withdrawals { "warn" } else { "ok" },
                "detail": if restrictions.enable_withdrawals { "Withdrawals enabled (not required, consider disabling)" } else { "Withdrawals disabled (recommended)" }
            }));

            permissions.push(serde_json::json!({
                "name": "internal_transfer",
                "label": "Internal Transfer",
                "status": if restrictions.enable_internal_transfer { "warn" } else { "ok" },
                "detail": if restrictions.enable_internal_transfer { "Internal transfer enabled (not required, consider disabling)" } else { "Internal transfer disabled (recommended)" }
            }));

            permissions.push(serde_json::json!({
                "name": "ip_restriction",
                "label": "IP Restriction",
                "status": if restrictions.ip_restrict { "ok" } else { "warn" },
                "detail": if restrictions.ip_restrict { "IP restriction enabled" } else { "IP restriction not enabled (consider enabling)" }
            }));

            Ok(Json(ApiResponse::ok(serde_json::json!({
                "permissions": permissions,
            }))))
        }
        Err(e) => {
            Ok(Json(ApiResponse::ok(serde_json::json!({
                "permissions": [{
                    "name": "connectivity",
                    "label": "Connectivity",
                    "status": "error",
                    "detail": format!("Failed to verify: {}", e)
                }],
            }))))
        }
    }
}

/// POST /api/credentials/verify — verify API key permissions via apiRestrictions.
/// Uses the exchange already registered in the registry (saved by save_credential).
pub async fn verify_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let _user_id = extract_user_id(&headers)?;

    let names = state.exchange_registry.registered_names();
    let exchange_ref = names.first().and_then(|key| state.exchange_registry.get(key));

    let exchange = match exchange_ref {
        Some(e) => e,
        None => return Ok(Json(ApiResponse::err("No exchange registered. Please save credentials first."))),
    };

    // Call apiRestrictions
    match exchange.get_api_restrictions().await {
        Ok(restrictions) => {
            tracing::info!(
                ip_restrict = restrictions.ip_restrict,
                ip_not_restricted = restrictions.ip_not_restricted,
                ip_whitelist = ?restrictions.ip_whitelist,
                read_info = restrictions.read_info,
                enable_spot_and_margin_trading = restrictions.enable_spot_and_margin_trading,
                enable_futures = restrictions.enable_futures,
                enable_withdrawals = restrictions.enable_withdrawals,
                enable_internal_transfer = restrictions.enable_internal_transfer,
                "apiRestrictions parsed result"
            );
            let mut permissions = Vec::new();

            // Note: "connectivity" is NOT included here — it's already checked
            // by the separate /test (ping) endpoint.

            permissions.push(serde_json::json!({
                "name": "read_info",
                "label": "Read Info",
                "status": if restrictions.read_info { "ok" } else { "error" },
                "detail": if restrictions.read_info { "Reading account info enabled" } else { "Reading account info disabled" }
            }));

            permissions.push(serde_json::json!({
                "name": "spot_trading",
                "label": "Spot & Margin Trading",
                "status": if restrictions.enable_spot_and_margin_trading { "ok" } else { "error" },
                "detail": if restrictions.enable_spot_and_margin_trading { "Spot and margin trading enabled" } else { "Spot and margin trading disabled" }
            }));

            permissions.push(serde_json::json!({
                "name": "futures",
                "label": "Futures Trading",
                "status": if restrictions.enable_futures { "ok" } else { "error" },
                "detail": if restrictions.enable_futures { "Futures trading enabled" } else { "Futures trading disabled" }
            }));

            permissions.push(serde_json::json!({
                "name": "withdrawals",
                "label": "Withdrawals",
                "status": if restrictions.enable_withdrawals { "warn" } else { "ok" },
                "detail": if restrictions.enable_withdrawals { "Withdrawals enabled (not required, consider disabling)" } else { "Withdrawals disabled (recommended)" }
            }));

            permissions.push(serde_json::json!({
                "name": "internal_transfer",
                "label": "Internal Transfer",
                "status": if restrictions.enable_internal_transfer { "warn" } else { "ok" },
                "detail": if restrictions.enable_internal_transfer { "Internal transfer enabled (not required, consider disabling)" } else { "Internal transfer disabled (recommended)" }
            }));

            permissions.push(serde_json::json!({
                "name": "ip_restriction",
                "label": "IP Restriction",
                "status": if restrictions.ip_restrict { "ok" } else { "warn" },
                "detail": if restrictions.ip_restrict { "IP restriction enabled" } else { "IP restriction not enabled (consider enabling)" }
            }));

            Ok(Json(ApiResponse::ok(serde_json::json!({
                "connected": true,
                "permissions": permissions,
            }))))
        }
        Err(e) => {
            Ok(Json(ApiResponse::ok(serde_json::json!({
                "connected": false,
                "permissions": [{
                    "name": "connectivity",
                    "label": "Connectivity",
                    "status": "error",
                    "detail": format!("Failed to verify: {}", e)
                }],
            }))))
        }
    }
}

/// GET /api/credentials/status — check if user has exchange credentials configured
pub async fn exchange_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let user_id = extract_user_id(&headers)?;

    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT exchange, market_type FROM qd_exchange_credentials WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    match row {
        Some((exchange, market_type)) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": true,
            "exchange": format!("{} ({})", exchange, market_type),
        })))),
        None => Ok(Json(ApiResponse::ok(serde_json::json!({
        "connected": false,
    })))),
    }
}
