use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::AppState;
use crate::api::middleware::AuthUser;
use crate::models::*;
use crate::utils::crypto;
use crate::trading::exchange::ExchangeFactory;

#[derive(Debug, Serialize, Deserialize)]
pub struct CredentialRequest {
    pub exchange: String,
    pub api_key: String,
    pub api_secret: String,
    pub passphrase: Option<String>,
    pub label: Option<String>,
    #[serde(default = "default_market_type")]
    pub market_type: MarketType,
}

fn default_market_type() -> MarketType {
    MarketType::Perpetual
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CredentialRow {
    pub id: Uuid,
    pub exchange: String,
    pub label: Option<String>,
    pub market_type: MarketType,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SavedCredentialRow {
    pub id: Uuid,
    pub exchange: String,
    pub label: Option<String>,
    pub market_type: MarketType,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_credentials(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let rows = sqlx::query_as::<_, CredentialRow>(
        r#"SELECT id, exchange, label, market_type, created_at FROM qd_exchange_credentials
           WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
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
    Json(req): Json<CredentialRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let encryption_key = crypto::derive_key(&state.config.server.encryption_key);
    let encrypted_key = crypto::encrypt(&req.api_key, &encryption_key).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Encryption error: {}", e))),
        )
    })?;
    let encrypted_secret = crypto::encrypt(&req.api_secret, &encryption_key).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Encryption error: {}", e))),
        )
    })?;
    let encrypted_passphrase = if let Some(ref pass) = req.passphrase {
        Some(crypto::encrypt(pass, &encryption_key).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Encryption error: {}", e))),
            )
        })?)
    } else {
        None
    };

    let market_type_str = match req.market_type {
        MarketType::Spot => "spot",
        MarketType::Perpetual => "perpetual",
    };

    let row = sqlx::query_as::<_, SavedCredentialRow>(
        r#"INSERT INTO qd_exchange_credentials
           (user_id, exchange, encrypted_api_key, encrypted_api_secret, encrypted_passphrase, label, market_type)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           ON CONFLICT (user_id, exchange, market_type) DO UPDATE SET
           encrypted_api_key = $3, encrypted_api_secret = $4, encrypted_passphrase = $5, label = $6, updated_at = NOW()
           RETURNING id, exchange, label, market_type, created_at"#,
    )
    .bind(user_id)
    .bind(&req.exchange)
    .bind(&encrypted_key)
    .bind(&encrypted_secret)
    .bind(&encrypted_passphrase)
    .bind(&req.label)
    .bind(&market_type_str)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Failed to save credentials: {}", e))),
        )
    })?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "id": row.id,
        "exchange": row.exchange,
        "label": row.label,
        "market_type": row.market_type,
    }))))
}

pub async fn delete_credential(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let result = sqlx::query("DELETE FROM qd_exchange_credentials WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Delete failed: {}", e))),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("Credential not found")),
        ));
    }

    Ok(Json(ApiResponse::ok_with_message(serde_json::json!({"id": id}), "Credential deleted")))
}

pub async fn test_credential(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(req): Json<CredentialRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let exchange = ExchangeFactory::create(
        &req.exchange,
        &req.api_key,
        &req.api_secret,
        req.passphrase.as_deref(),
        state.config.proxy.as_deref(),
        req.market_type,
    )
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err(format!("Failed to create exchange: {}", e))),
        )
    })?;

    match exchange.ping().await {
        Ok(true) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "exchange": req.exchange,
            "connected": true,
            "message": format!("Successfully connected to {}", req.exchange),
        })))),
        Ok(false) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "exchange": req.exchange,
            "connected": false,
            "message": format!("{} ping returned false", req.exchange),
        })))),
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Connection test failed for {}: {}", req.exchange, e
            ))),
        )),
    }
}
