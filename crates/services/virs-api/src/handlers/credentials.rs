//! Exchange credentials handlers.

use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use virs_error::VirsError;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

pub async fn list_credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let creds = sqlx::query_as::<_, (uuid::Uuid, String, String, String, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT id, exchange, label, market_type, created_at FROM qd_exchange_credentials WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": creds.iter().map(|(id, exchange, label, market_type, created_at)| {
            serde_json::json!({
                "id": id.to_string(),
                "exchange": exchange,
                "label": label,
                "market_type": market_type,
                "created_at": created_at.to_rfc3339(),
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

    let exchange = body["exchange"].as_str().unwrap_or("");
    let label = body["label"].as_str().unwrap_or("");
    let api_key = body["api_key"].as_str().unwrap_or("");
    let api_secret = body["api_secret"].as_str().unwrap_or("");
    let passphrase = body["passphrase"].as_str();
    let market_type = body["market_type"].as_str().unwrap_or("perpetual");

    if exchange.is_empty() || api_key.is_empty() || api_secret.is_empty() {
        return Err(VirsError::bad_request(
            "exchange, api_key, api_secret are required",
        ));
    }

    let id = uuid::Uuid::new_v4();

    // Encrypt sensitive fields with AES-256-GCM
    let encrypted_api_key =
        virs_utils::crypto::encrypt_with_key(api_key, &state.encryption_key)?;
    let encrypted_secret =
        virs_utils::crypto::encrypt_with_key(api_secret, &state.encryption_key)?;
    let encrypted_passphrase = passphrase
        .map(|p| virs_utils::crypto::encrypt_with_key(p, &state.encryption_key))
        .transpose()?;

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
    .await?;

    // Auto-register exchange in registry for immediate use
    let mt = match market_type {
        "spot" => virs_ccxt::MarketType::Spot,
        _ => virs_ccxt::MarketType::Perpetual,
    };
    if let Ok(ccxt_ex) = virs_ccxt::create_exchange(
        exchange,
        api_key,
        api_secret,
        passphrase,
        None,
        &mt,
        std::time::Duration::from_secs(state.http_timeout_secs),
        std::time::Duration::from_secs(state.http_connect_timeout_secs),
        state.http_pool_max_idle_per_host,
        state.listenkey_keepalive_futures_secs,
        state.listenkey_keepalive_spot_secs,
        state.ws_reconnect_initial_delay_secs,
        state.ws_reconnect_max_delay_secs,
        state.ws_ping_interval_secs,
        state.ws_max_lifetime_secs,
    ) {
        // 同步服务器时间，校准签名时间戳偏移（非阻塞 — 失败仅告警）
        if let Err(e) = ccxt_ex.sync_time().await {
            tracing::warn!(
                error = %e,
                exchange,
                market_type,
                "Server time sync failed, using local clock (recvWindow 5000ms tolerates small drift)"
            );
        }
        let app_mt = match market_type {
            "spot" => virs_models::MarketType::Spot,
            _ => virs_models::MarketType::Perpetual,
        };
        let adapter = virs_exchange::CcxtAdapter::new(ccxt_ex, app_mt);
        state.exchange_registry.register(Box::new(adapter));
    }

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

    sqlx::query(r#"DELETE FROM qd_exchange_credentials WHERE id = $1 AND user_id = $2"#)
        .bind(id)
        .bind(user_id)
        .execute(&state.db_pool)
        .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true}))))
}

/// POST /api/credentials/test — test connectivity only (ping).
/// Uses the exchange already registered in the registry (saved by save_credential).
pub async fn test_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let names = state.exchange_registry.registered_names();
    let exchange_ref = names
        .first()
        .and_then(|key| state.exchange_registry.get(key));

    let exchange = match exchange_ref {
        Some(e) => e,
        None => {
            return Ok(Json(ApiResponse::ok(serde_json::json!({
                "connected": false,
                "message": "No exchange registered. Please save credentials first.",
            }))))
        }
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
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let names = state.exchange_registry.registered_names();
    let exchange_ref = names
        .first()
        .and_then(|key| state.exchange_registry.get(key));

    let exchange = match exchange_ref {
        Some(e) => e,
        None => {
            return Ok(Json(ApiResponse::ok(serde_json::json!({
                "permissions": [{
                    "name": "connectivity",
                    "label": "Connectivity",
                    "status": "error",
                    "detail": "No exchange registered. Please save credentials first.",
                }],
            }))))
        }
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
        Err(e) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "permissions": [{
                "name": "connectivity",
                "label": "Connectivity",
                "status": "error",
                "detail": format!("Failed to verify: {}", e)
            }],
        })))),
    }
}

/// POST /api/credentials/verify — verify API key permissions via apiRestrictions.
/// Uses the exchange already registered in the registry (saved by save_credential).
pub async fn verify_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let names = state.exchange_registry.registered_names();
    let exchange_ref = names
        .first()
        .and_then(|key| state.exchange_registry.get(key));

    let exchange = match exchange_ref {
        Some(e) => e,
        None => {
            return Err(VirsError::bad_request(
                "No exchange registered. Please save credentials first.",
            ))
        }
    };

    // Call apiRestrictions
    match exchange.get_api_restrictions().await {
        Ok(restrictions) => {
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
        Err(e) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": false,
            "permissions": [{
                "name": "connectivity",
                "label": "Connectivity",
                "status": "error",
                "detail": format!("Failed to verify: {}", e)
            }],
        })))),
    }
}

/// GET /api/credentials/position-mode — query the exchange's current position mode.
/// Returns { supported: bool, mode: "hedge"|"oneway"|null }.
/// - Hedge:  { supported: true, mode: "hedge" }
/// - OneWay: { supported: true, mode: "oneway" }  (fapi returns Err for OneWay — caught here)
/// - Spot:   { supported: false, mode: null }     (get_position_mode returns NotSupported)
pub async fn check_position_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let names = state.exchange_registry.registered_names();
    let exchange_ref = names
        .first()
        .and_then(|key| state.exchange_registry.get(key));

    let exchange = match exchange_ref {
        Some(e) => e,
        None => {
            return Ok(Json(ApiResponse::ok(serde_json::json!({
                "supported": false,
                "mode": null,
                "message": "No exchange registered. Please save credentials first.",
            }))))
        }
    };

    match exchange.get_position_mode().await {
        Ok(_) => {
            // Only Hedge reaches Ok — fapi returns Err for OneWay.
            Ok(Json(ApiResponse::ok(serde_json::json!({
                "supported": true,
                "mode": "hedge",
            }))))
        }
        Err(e) => {
            let err_str = format!("{}", e);
            // OneWay mode — fapi returns InvalidRequest with "OneWay" in the message.
            // Report it to the frontend so the wizard can block.
            if err_str.contains("OneWay") || err_str.contains("oneway") {
                Ok(Json(ApiResponse::ok(serde_json::json!({
                    "supported": true,
                    "mode": "oneway",
                }))))
            } else if err_str.contains("Not supported") || err_str.contains("not supported") {
                // Spot exchanges don't support position mode.
                Ok(Json(ApiResponse::ok(serde_json::json!({
                    "supported": false,
                    "mode": null,
                }))))
            } else {
                Ok(Json(ApiResponse::ok(serde_json::json!({
                    "supported": false,
                    "mode": null,
                    "message": err_str,
                }))))
            }
        }
    }
}

/// GET /api/credentials/status — check if user has exchange credentials configured
pub async fn exchange_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT exchange, market_type FROM qd_exchange_credentials WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await?;

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
