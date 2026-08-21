use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use virs_error::VirsError;
use virs_database as db;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

pub async fn list_credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let creds = db::list_exchange_credentials(&state.db_pool, user_id).await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": creds.iter().map(|(id, exchange, label, created_at)| {
            serde_json::json!({
                "id": id.to_string(),
                "exchange": exchange,
                "label": label,
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

    let exchange = body["exchange"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("exchange is required"))?;
    let label = body["label"]
        .as_str()
        .ok_or_else(|| VirsError::bad_request("label is required"))?;
    let api_key = body["api_key"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("api_key is required"))?;
    let api_secret = body["api_secret"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("api_secret is required"))?;
    let passphrase = body["passphrase"].as_str();

    let id = uuid::Uuid::new_v4();


    /* 保存凭据时加密API Key/Secret/Passphrase，使用AES加密后存入数据库 */
    let encrypted_api_key =
        virs_utils::encrypt_with_key(api_key, &state.encryption_key)?;
    let encrypted_secret =
        virs_utils::encrypt_with_key(api_secret, &state.encryption_key)?;
    let encrypted_passphrase = passphrase
        .map(|p| virs_utils::encrypt_with_key(p, &state.encryption_key))
        .transpose()?;

    db::save_exchange_credential(
        &state.db_pool,
        id,
        user_id,
        exchange,
        label,
        &encrypted_api_key,
        &encrypted_secret,
        encrypted_passphrase.as_deref(),
    )
    .await?;


    if let Ok(exchange) = virs_ccxt::create_exchange(
        exchange,
        api_key,
        api_secret,
        passphrase,
        None,
        std::time::Duration::from_secs(state.http_timeout_secs),
        std::time::Duration::from_secs(state.http_connect_timeout_secs),
        state.http_pool_max_idle_per_host,
        state.listenkey_keepalive_futures_secs,
    )
    .await
    {
        state.exchange_registry.register(exchange);
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

    db::delete_exchange_credential(&state.db_pool, id, user_id).await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true}))))
}


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


    match exchange.get_api_restrictions().await {
        Ok(restrictions) => {
            let mut permissions = Vec::new();


            permissions.push(serde_json::json!({
                "name": "read_info",
                "label": "Read Info",
                "status": match restrictions.read_info {
                    Some(true) => "ok",
                    Some(false) => "error",
                    None => "unknown",
                },
                "detail": match restrictions.read_info {
                    Some(true) => "Reading account info enabled",
                    Some(false) => "Reading account info disabled",
                    None => "Reading account info status unknown (API did not return enableReading)",
                }
            }));

            permissions.push(serde_json::json!({
                "name": "futures",
                "label": "Futures Trading",
                "status": if restrictions.enable_futures == Some(true) { "ok" } else { "error" },
                "detail": if restrictions.enable_futures == Some(true) { "Futures trading enabled" } else { "Futures trading disabled" }
            }));

            permissions.push(serde_json::json!({
                "name": "withdrawals",
                "label": "Withdrawals",
                "status": if restrictions.enable_withdrawals == Some(true) { "warn" } else { "ok" },
                "detail": if restrictions.enable_withdrawals == Some(true) { "Withdrawals enabled (not required, consider disabling)" } else { "Withdrawals disabled (recommended)" }
            }));

            permissions.push(serde_json::json!({
                "name": "internal_transfer",
                "label": "Internal Transfer",
                "status": if restrictions.enable_internal_transfer == Some(true) { "warn" } else { "ok" },
                "detail": if restrictions.enable_internal_transfer == Some(true) { "Internal transfer enabled (not required, consider disabling)" } else { "Internal transfer disabled (recommended)" }
            }));

            permissions.push(serde_json::json!({
                "name": "ip_restriction",
                "label": "IP Restriction",
                "status": if restrictions.ip_restrict == Some(true) { "ok" } else { "warn" },
                "detail": if restrictions.ip_restrict == Some(true) { "IP restriction enabled" } else { "IP restriction not enabled (consider enabling)" }
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
                "detail": format!("{}", e)
            }],
        })))),
    }
}


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


    match exchange.get_api_restrictions().await {
        Ok(restrictions) => {
            let mut permissions = Vec::new();


            permissions.push(serde_json::json!({
                "name": "read_info",
                "label": "Read Info",
                "status": match restrictions.read_info {
                    Some(true) => "ok",
                    Some(false) => "error",
                    None => "unknown",
                },
                "detail": match restrictions.read_info {
                    Some(true) => "Reading account info enabled",
                    Some(false) => "Reading account info disabled",
                    None => "Reading account info status unknown (API did not return enableReading)",
                }
            }));

            permissions.push(serde_json::json!({
                "name": "futures",
                "label": "Futures Trading",
                "status": if restrictions.enable_futures == Some(true) { "ok" } else { "error" },
                "detail": if restrictions.enable_futures == Some(true) { "Futures trading enabled" } else { "Futures trading disabled" }
            }));

            permissions.push(serde_json::json!({
                "name": "withdrawals",
                "label": "Withdrawals",
                "status": if restrictions.enable_withdrawals == Some(true) { "warn" } else { "ok" },
                "detail": if restrictions.enable_withdrawals == Some(true) { "Withdrawals enabled (not required, consider disabling)" } else { "Withdrawals disabled (recommended)" }
            }));

            permissions.push(serde_json::json!({
                "name": "internal_transfer",
                "label": "Internal Transfer",
                "status": if restrictions.enable_internal_transfer == Some(true) { "warn" } else { "ok" },
                "detail": if restrictions.enable_internal_transfer == Some(true) { "Internal transfer enabled (not required, consider disabling)" } else { "Internal transfer disabled (recommended)" }
            }));

            permissions.push(serde_json::json!({
                "name": "ip_restriction",
                "label": "IP Restriction",
                "status": if restrictions.ip_restrict == Some(true) { "ok" } else { "warn" },
                "detail": if restrictions.ip_restrict == Some(true) { "IP restriction enabled" } else { "IP restriction not enabled (consider enabling)" }
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
                "detail": format!("{}", e)
            }],
        })))),
    }
}


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

    /* 仅支持Hedge（双向持仓）模式：OneWay模式被标记为不支持，系统不接受OneWay模式 */
    match exchange.get_position_mode().await {
        Ok(_) => {

            Ok(Json(ApiResponse::ok(serde_json::json!({
                "supported": true,
                "mode": "hedge",
            }))))
        }
        Err(e) => {
            let err_str = format!("{}", e);


            if err_str.contains("OneWay") || err_str.contains("oneway") {
                Ok(Json(ApiResponse::ok(serde_json::json!({
                    "supported": true,
                    "mode": "oneway",
                }))))
            } else if err_str.contains("Not supported") || err_str.contains("not supported") {

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


pub async fn exchange_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let exchange = db::get_user_exchange(&state.db_pool, user_id).await?;

    match exchange {
        Some(exchange) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": true,
            "exchange": exchange,
        })))),
        None => Ok(Json(ApiResponse::ok(serde_json::json!({
            "connected": false,
        })))),
    }
}
