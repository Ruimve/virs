use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::api::AppState;
use crate::api::middleware::OptionalAuthUser;
use crate::exchange::ExchangeFactory;
use crate::models::*;
use crate::utils::crypto;

/// Ensure an exchange instance is available for the given exchange name.
/// Tries the engine cache first, then loads credentials from the database.
/// Returns the exchange name key to use (may be user-scoped).
pub async fn ensure_exchange(
    state: &Arc<AppState>,
    exchange_name: &str,
) -> Result<String, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    // Check if already registered globally
    if state.strategy_engine.get_exchange(exchange_name).is_some() {
        return Ok(exchange_name.to_string());
    }

    // Try to load credentials from database (any user's credentials for public data)
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        r#"SELECT encrypted_api_key, encrypted_api_secret, encrypted_passphrase
           FROM qd_exchange_credentials
           WHERE exchange = $1 LIMIT 1"#,
    )
    .bind(exchange_name)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Database error: {}", e
            ))),
        )
    })?;

    match row {
        Some((enc_key, enc_secret, enc_passphrase)) => {
            let encryption_key = crypto::derive_key(&state.config.server.encryption_key);
            let api_key = crypto::decrypt(&enc_key, &encryption_key).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<serde_json::Value>::err(format!(
                        "Failed to decrypt API key: {}", e
                    ))),
                )
            })?;
            let api_secret = crypto::decrypt(&enc_secret, &encryption_key).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<serde_json::Value>::err(format!(
                        "Failed to decrypt API secret: {}", e
                    ))),
                )
            })?;
            let passphrase = enc_passphrase
                .and_then(|p| crypto::decrypt(&p, &encryption_key).ok());

            let exchange = ExchangeFactory::create(
                exchange_name,
                &api_key,
                &api_secret,
                passphrase.as_deref(),
                state.config.proxy.as_deref(),
            )
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<serde_json::Value>::err(format!(
                        "Failed to create exchange '{}': {}", exchange_name, e
                    ))),
                )
            })?;

            state.strategy_engine.register_exchange(exchange);
            Ok(exchange_name.to_string())
        }
        None => Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Exchange '{}' has no credentials configured. Please add API keys in the Credentials page.",
                exchange_name
            ))),
        )),
    }
}

#[derive(Deserialize)]
pub struct TickerQuery {
    symbol: String,
    exchange: Option<String>,
}

pub async fn get_ticker(
    State(state): State<Arc<AppState>>,
    _auth: OptionalAuthUser,
    Query(params): Query<TickerQuery>,
) -> Result<Json<ApiResponse<Ticker>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let exchange_name = params.exchange.as_deref().unwrap_or("binance");
    let exchange_key = ensure_exchange(&state, exchange_name).await?;
    let exchange = state.strategy_engine.get_exchange(&exchange_key).unwrap();

    match exchange.get_ticker(&params.symbol).await {
        Ok(ticker) => Ok(Json(ApiResponse::ok(ticker))),
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to fetch ticker for {} from {}: {}",
                params.symbol, exchange_name, e
            ))),
        )),
    }
}

#[derive(Deserialize)]
pub struct KlineQuery {
    symbol: String,
    interval: String,
    exchange: Option<String>,
    limit: Option<u32>,
    end_time: Option<i64>,
}

pub async fn get_klines(
    State(state): State<Arc<AppState>>,
    _auth: OptionalAuthUser,
    Query(params): Query<KlineQuery>,
) -> Result<Json<ApiResponse<Vec<Kline>>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let exchange_name = params.exchange.as_deref().unwrap_or("binance");
    let limit = params.limit.unwrap_or(200);

    let exchange_key = ensure_exchange(&state, exchange_name).await?;
    let exchange = state.strategy_engine.get_exchange(&exchange_key).unwrap();

    match exchange.get_klines(&params.symbol, &params.interval, limit, params.end_time).await {
        Ok(klines) => {
            if klines.is_empty() {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(ApiResponse::<serde_json::Value>::err(format!(
                        "No kline data available for {} ({}) on {}. The symbol may not exist or no data for this timeframe.",
                        params.symbol, params.interval, exchange_name
                    ))),
                ))
            } else {
                Ok(Json(ApiResponse::ok(klines)))
            }
        }
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to fetch klines for {} from {}: {}",
                params.symbol, exchange_name, e
            ))),
        )),
    }
}

#[derive(Deserialize)]
pub struct OrderBookQuery {
    symbol: String,
    exchange: Option<String>,
    depth: Option<u32>,
}

pub async fn get_order_book(
    State(state): State<Arc<AppState>>,
    _auth: OptionalAuthUser,
    Query(params): Query<OrderBookQuery>,
) -> Result<Json<ApiResponse<OrderBook>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let exchange_name = params.exchange.as_deref().unwrap_or("binance");
    let depth = params.depth.unwrap_or(20);

    let exchange_key = ensure_exchange(&state, exchange_name).await?;
    let exchange = state.strategy_engine.get_exchange(&exchange_key).unwrap();

    match exchange.get_order_book(&params.symbol, depth).await {
        Ok(order_book) => {
            if order_book.bids.is_empty() && order_book.asks.is_empty() {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(ApiResponse::<serde_json::Value>::err(format!(
                        "No order book data for {} on {}. The symbol may not exist.",
                        params.symbol, exchange_name
                    ))),
                ))
            } else {
                Ok(Json(ApiResponse::ok(order_book)))
            }
        }
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to fetch order book for {} from {}: {}",
                params.symbol, exchange_name, e
            ))),
        )),
    }
}

pub async fn get_balances(
    State(state): State<Arc<AppState>>,
    auth: OptionalAuthUser,
) -> Result<Json<ApiResponse<Vec<Balance>>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    // Balances require authentication
    let _user = match auth.0 {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<serde_json::Value>::err(
                    "Authentication required to view balances. Please login first.",
                )),
            ));
        }
    };

    // Try to get balances from the first available exchange in the database
    let exchange_name = "binance"; // default
    let exchange_key = ensure_exchange(&state, exchange_name).await?;
    let exchange = state.strategy_engine.get_exchange(&exchange_key).unwrap();

    match exchange.get_balances().await {
        Ok(balances) => {
            if balances.is_empty() {
                Ok(Json(ApiResponse::ok_with_message(vec![], "No non-zero balances found")))
            } else {
                Ok(Json(ApiResponse::ok(balances)))
            }
        }
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to fetch balances from {}: {}",
                exchange_name, e
            ))),
        )),
    }
}

#[derive(Deserialize)]
pub struct SymbolsQuery {
    exchange: Option<String>,
}

pub async fn get_symbols(
    State(state): State<Arc<AppState>>,
    _auth: OptionalAuthUser,
    Query(params): Query<SymbolsQuery>,
) -> Result<Json<ApiResponse<Vec<String>>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let exchange_name = params.exchange.as_deref().unwrap_or("binance");

    let exchange_key = ensure_exchange(&state, exchange_name).await?;
    let exchange = state.strategy_engine.get_exchange(&exchange_key).unwrap();

    match exchange.get_symbols(MarketType::Spot).await {
        Ok(symbols) => {
            if symbols.is_empty() {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(ApiResponse::<serde_json::Value>::err(format!(
                        "No trading symbols available from {}. The exchange may be unreachable.",
                        exchange_name
                    ))),
                ))
            } else {
                Ok(Json(ApiResponse::ok(symbols)))
            }
        }
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to fetch symbols from {}: {}",
                exchange_name, e
            ))),
        )),
    }
}
