use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::AppState;
use crate::api::middleware::AuthUser;
use crate::exchange::ExchangeFactory;
use crate::models::*;
use crate::utils::crypto;

enum StrategyUpdateParam {
    Text(String),
    I64(i64),
    Json(serde_json::Value),
}

#[derive(Deserialize)]
pub struct StrategyListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    status: Option<String>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct StrategyOwnerRow {
    pub user_id: Uuid,
}

pub async fn list_strategies(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<StrategyListQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let (page, page_size) = (params.page.unwrap_or(1), params.page_size.unwrap_or(20));
    let offset = (page - 1) * page_size;

    let user_id = if auth.is_admin_or_manager() {
        None
    } else {
        Some(auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?)
    };

    let strategies = if let Some(uid) = user_id {
        sqlx::query_as::<_, Strategy>(
            r#"SELECT id, user_id, name, description, strategy_type,
               market_type, symbol, exchange, timeframe,
               strategy_mode, execution_mode,
               indicator_config, trading_config, exchange_config, notification_config,
               strategy_code, decide_interval_secs, status,
               created_at, updated_at
               FROM qd_strategies_trading WHERE user_id = $1
               ORDER BY created_at DESC LIMIT $2 OFFSET $3"#,
        )
        .bind(uid)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&state.db_pool)
        .await
    } else {
        sqlx::query_as::<_, Strategy>(
            r#"SELECT id, user_id, name, description, strategy_type,
               market_type, symbol, exchange, timeframe,
               strategy_mode, execution_mode,
               indicator_config, trading_config, exchange_config, notification_config,
               strategy_code, decide_interval_secs, status,
               created_at, updated_at
               FROM qd_strategies_trading
               ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
        )
        .bind(page_size)
        .bind(offset)
        .fetch_all(&state.db_pool)
        .await
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let total: i64 = if let Some(uid) = user_id {
        sqlx::query_scalar("SELECT COUNT(*) FROM qd_strategies_trading WHERE user_id = $1")
            .bind(uid)
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0)
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM qd_strategies_trading")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0)
    };

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": strategies,
        "total": total,
        "page": page,
        "page_size": page_size,
        "total_pages": (total + page_size - 1) / page_size,
    }))))
}

pub async fn create_strategy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<CreateStrategy>,
) -> Result<Json<ApiResponse<Strategy>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let market_type_str = match req.market_type {
        MarketType::Spot => "spot",
        MarketType::Perpetual => "perpetual",
    };
    let strategy_mode_str = match req.strategy_mode {
        StrategyMode::Signal => "signal",
        StrategyMode::Script => "script",
    };
    let execution_mode_str = match req.execution_mode {
        ExecutionMode::SignalOnly => "signal_only",
        ExecutionMode::Live => "live",
    };

    let strategy = sqlx::query_as::<_, Strategy>(
        r#"INSERT INTO qd_strategies_trading
           (user_id, name, description, strategy_type, market_type, symbol, exchange,
            timeframe, strategy_mode, execution_mode, indicator_config, trading_config,
            exchange_config, notification_config, strategy_code, decide_interval_secs, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
           RETURNING id, user_id, name, description, strategy_type,
           market_type, symbol, exchange, timeframe,
           strategy_mode, execution_mode,
           indicator_config, trading_config, exchange_config, notification_config,
           strategy_code, decide_interval_secs, status,
           created_at, updated_at"#,
    )
    .bind(user_id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.strategy_type)
    .bind(market_type_str)
    .bind(&req.symbol)
    .bind(&req.exchange)
    .bind(&req.timeframe)
    .bind(strategy_mode_str)
    .bind(execution_mode_str)
    .bind(&req.indicator_config)
    .bind(&req.trading_config)
    .bind(&req.exchange_config)
    .bind(&req.notification_config)
    .bind(&req.strategy_code)
    .bind(req.decide_interval_secs.unwrap_or(300))
    .bind("draft")
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Failed to create strategy: {}", e))),
        )
    })?;

    Ok(Json(ApiResponse::ok(strategy)))
}

pub async fn get_strategy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Strategy>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let strategy = sqlx::query_as::<_, Strategy>(
        r#"SELECT id, user_id, name, description, strategy_type,
           market_type, symbol, exchange, timeframe,
           strategy_mode, execution_mode,
           indicator_config, trading_config, exchange_config, notification_config,
           strategy_code, decide_interval_secs, status,
           created_at, updated_at
           FROM qd_strategies_trading WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    match strategy {
        Some(s) => {
            if !auth.is_admin_or_manager() && s.user_id.to_string() != auth.user_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ApiResponse::<serde_json::Value>::err("Access denied")),
                ));
            }
            Ok(Json(ApiResponse::ok(s)))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("Strategy not found")),
        )),
    }
}

pub async fn update_strategy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let existing = sqlx::query_as::<_, StrategyOwnerRow>(
        r#"SELECT user_id FROM qd_strategies_trading WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    match existing {
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<serde_json::Value>::err("Strategy not found")),
            ));
        }
        Some(s) => {
            if !auth.is_admin_or_manager() && s.user_id.to_string() != auth.user_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ApiResponse::<serde_json::Value>::err("Access denied")),
                ));
            }
        }
    }

    let mut set_clauses: Vec<String> = Vec::new();
    let mut params: Vec<StrategyUpdateParam> = Vec::new();
    let mut bind_idx = 1;

    if let Some(value) = req.get("name").and_then(|v| v.as_str()) {
        set_clauses.push(format!("name = ${}", bind_idx));
        params.push(StrategyUpdateParam::Text(value.to_string()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("description").and_then(|v| v.as_str()) {
        set_clauses.push(format!("description = ${}", bind_idx));
        params.push(StrategyUpdateParam::Text(value.to_string()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("symbol").and_then(|v| v.as_str()) {
        set_clauses.push(format!("symbol = ${}", bind_idx));
        params.push(StrategyUpdateParam::Text(value.to_string()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("exchange").and_then(|v| v.as_str()) {
        set_clauses.push(format!("exchange = ${}", bind_idx));
        params.push(StrategyUpdateParam::Text(value.to_string()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("timeframe").and_then(|v| v.as_str()) {
        set_clauses.push(format!("timeframe = ${}", bind_idx));
        params.push(StrategyUpdateParam::Text(value.to_string()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("indicator_config") {
        set_clauses.push(format!("indicator_config = ${}", bind_idx));
        params.push(StrategyUpdateParam::Json(value.clone()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("trading_config") {
        set_clauses.push(format!("trading_config = ${}", bind_idx));
        params.push(StrategyUpdateParam::Json(value.clone()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("notification_config") {
        set_clauses.push(format!("notification_config = ${}", bind_idx));
        params.push(StrategyUpdateParam::Json(value.clone()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("strategy_code").and_then(|v| v.as_str()) {
        set_clauses.push(format!("strategy_code = ${}", bind_idx));
        params.push(StrategyUpdateParam::Text(value.to_string()));
        bind_idx += 1;
    }
    if let Some(value) = req.get("decide_interval_secs").and_then(|v| v.as_i64()).map(|v| v as i32) {
        set_clauses.push(format!("decide_interval_secs = ${}", bind_idx));
        params.push(StrategyUpdateParam::I64(value as i64));
        bind_idx += 1;
    }

    if set_clauses.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("No fields to update")),
        ));
    }

    set_clauses.push(format!("updated_at = NOW()"));

    let query_str = format!(
        "UPDATE qd_strategies_trading SET {} WHERE id = ${}",
        set_clauses.join(", "),
        bind_idx
    );

    let mut query = sqlx::query(&query_str);
    for param in &params {
        query = match param {
            StrategyUpdateParam::Text(v) => query.bind(v),
            StrategyUpdateParam::I64(v) => query.bind(*v),
            StrategyUpdateParam::Json(v) => query.bind(v),
        };
    }
    query = query.bind(id);

    query.execute(&state.db_pool).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Failed to update strategy: {}", e))),
        )
    })?;

    Ok(Json(ApiResponse::ok_with_message(serde_json::json!({"id": id}), "Strategy updated")))
}

pub async fn delete_strategy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let existing = sqlx::query_as::<_, StrategyOwnerRow>(
        r#"SELECT user_id FROM qd_strategies_trading WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    match existing {
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<serde_json::Value>::err("Strategy not found")),
            ));
        }
        Some(s) => {
            if !auth.is_admin_or_manager() && s.user_id.to_string() != auth.user_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ApiResponse::<serde_json::Value>::err("Access denied")),
                ));
            }
        }
    }

    state.strategy_engine.stop_strategy(&id);

    // Clean up user-scoped exchange instance before deleting strategy record
    let strategy_row: Option<(String, String, uuid::Uuid)> = sqlx::query_as(
        r#"SELECT exchange, market_type, user_id FROM qd_strategies_trading WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .ok()
    .flatten();

    if let Some((exchange, market_type, user_id)) = strategy_row {
        state.strategy_engine.remove_user_exchange(&exchange, &market_type, &user_id.to_string());
    }

    sqlx::query("DELETE FROM qd_strategies_trading WHERE id = $1")
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Delete failed: {}", e))),
            )
        })?;

    Ok(Json(ApiResponse::ok_with_message(serde_json::json!({"id": id}), "Strategy deleted")))
}

pub async fn start_strategy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let existing = sqlx::query_as::<_, StrategyOwnerRow>(
        r#"SELECT user_id FROM qd_strategies_trading WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    match existing {
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<serde_json::Value>::err("Strategy not found")),
            ));
        }
        Some(s) => {
            if !auth.is_admin_or_manager() && s.user_id.to_string() != auth.user_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ApiResponse::<serde_json::Value>::err("Access denied")),
                ));
            }
        }
    }

    sqlx::query("UPDATE qd_strategies_trading SET status = 'running', updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
            )
        })?;

    let strategy = sqlx::query_as::<_, Strategy>(
        r#"SELECT id, user_id, name, description, strategy_type,
           market_type, symbol, exchange, timeframe,
           strategy_mode, execution_mode,
           indicator_config, trading_config, exchange_config, notification_config,
           strategy_code, decide_interval_secs, status,
           created_at, updated_at
           FROM qd_strategies_trading WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Strategy not found: {}", e))),
        )
    })?;

    // Try to load user's exchange credentials from database based on strategy.market_type.
    // If found, create a user-scoped exchange instance.
    // Otherwise, fall back to the globally registered exchange.
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;
    let scoped_exchange_key = load_user_exchange(
        &state,
        &strategy.exchange,
        user_id,
        strategy.market_type.clone(),
    )
    .await;

    state
        .strategy_engine
        .start_strategy(strategy, scoped_exchange_key)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Failed to start strategy: {}", e))),
            )
        })?;

    Ok(Json(ApiResponse::ok_with_message(serde_json::json!({"id": id}), "Strategy started")))
}

pub async fn stop_strategy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let existing = sqlx::query_as::<_, StrategyOwnerRow>(
        r#"SELECT user_id FROM qd_strategies_trading WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    match existing {
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<serde_json::Value>::err("Strategy not found")),
            ));
        }
        Some(s) => {
            if !auth.is_admin_or_manager() && s.user_id.to_string() != auth.user_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ApiResponse::<serde_json::Value>::err("Access denied")),
                ));
            }
        }
    }

    state.strategy_engine.stop_strategy(&id);

    // Clean up user-scoped exchange instance
    let strategy_row: Option<(String, String, uuid::Uuid)> = sqlx::query_as(
        r#"SELECT exchange, market_type, user_id FROM qd_strategies_trading WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .ok()
    .flatten();

    if let Some((exchange, market_type, user_id)) = strategy_row {
        state.strategy_engine.remove_user_exchange(&exchange, &market_type, &user_id.to_string());
    }

    sqlx::query("UPDATE qd_strategies_trading SET status = 'stopped', updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
            )
        })?;

    Ok(Json(ApiResponse::ok_with_message(serde_json::json!({"id": id}), "Strategy stopped")))
}

/// Validate a Lua strategy script.
pub async fn validate_script(
    State(_state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let code = body.get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if code.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("Script code is empty")),
        ));
    }

    let executor = crate::engine::strategy::lua_executor::LuaExecutor::new(
        crate::engine::strategy::lua_executor::LuaExecutorConfig::default()
    );

    match executor.validate(code) {
        Ok(()) => Ok(Json(ApiResponse::ok_with_message(
            serde_json::json!({"valid": true}),
            "Script syntax is valid"
        ))),
        Err(e) => Ok(Json(ApiResponse::ok_with_message(
            serde_json::json!({"valid": false, "error": e}),
            "Script validation failed"
        ))),
    }
}

/// Try to load a user's exchange credentials from the database and register
/// a user-scoped exchange instance. Returns the scoped key on success,
/// or None if no credentials are found (falling back to .env config).
async fn load_user_exchange(
    state: &Arc<AppState>,
    exchange_name: &str,
    user_id: Uuid,
    market_type: MarketType,
) -> Option<String> {
    // Check if we already have a user-scoped exchange registered
    let scoped_key = format!("{}:{}:{}", exchange_name, market_type, user_id);
    if state.strategy_engine.get_exchange(&scoped_key).is_some() {
        return Some(scoped_key);
    }

    let market_type_str = match market_type {
        MarketType::Spot => "spot",
        MarketType::Perpetual => "perpetual",
    };

    // Query database for user's credentials with market_type filter
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        r#"SELECT encrypted_api_key, encrypted_api_secret, encrypted_passphrase
           FROM qd_exchange_credentials
           WHERE user_id = $1 AND exchange = $2 AND market_type = $3 LIMIT 1"#,
    )
    .bind(user_id)
    .bind(exchange_name)
    .bind(&market_type_str)
    .fetch_optional(&state.db_pool)
    .await
    .ok()?;

    let (enc_key, enc_secret, enc_passphrase) = row?;

    // Decrypt credentials
    let encryption_key = crypto::derive_key(&state.config.server.encryption_key);
    let api_key = crypto::decrypt(&enc_key, &encryption_key).ok()?;
    let api_secret = crypto::decrypt(&enc_secret, &encryption_key).ok()?;
    let passphrase = enc_passphrase
        .and_then(|p| crypto::decrypt(&p, &encryption_key).ok());

    // Create and register the exchange instance
    let exchange = ExchangeFactory::create(
        exchange_name,
        &api_key,
        &api_secret,
        passphrase.as_deref(),
        state.config.proxy.as_deref(),
        market_type,
    )
    .ok()?;

    let key = state
        .strategy_engine
        .register_exchange_for_user(exchange, user_id);

    tracing::info!(
        "Loaded credentials for '{}' ({}) from database for user {}",
        exchange_name, market_type_str, user_id
    );

    Some(key)
}
