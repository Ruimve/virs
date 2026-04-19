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
use crate::models::*;

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
        Some(Uuid::parse_str(&auth.user_id).unwrap_or(Uuid::nil()))
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
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or(Uuid::nil());

    let market_type_str = match req.market_type {
        MarketType::Spot => "spot",
        MarketType::Futures => "futures",
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
    if let Some(value) = req.get("decide_interval_secs").and_then(|v| v.as_i64()) {
        set_clauses.push(format!("decide_interval_secs = ${}", bind_idx));
        params.push(StrategyUpdateParam::I64(value));
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

    state
        .strategy_engine
        .start_strategy(strategy)
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
