use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::AppState;
use crate::api::middleware::AuthUser;
use crate::models::*;

pub async fn dashboard_summary(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let total_strategies: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM qd_strategies_trading WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let running_strategies: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM qd_strategies_trading WHERE user_id = $1 AND status = 'running'"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let open_positions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM qd_strategy_positions WHERE strategy_id IN (SELECT id FROM qd_strategies_trading WHERE user_id = $1) AND closed_at IS NULL"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let total_trades: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM qd_strategy_trades WHERE strategy_id IN (SELECT id FROM qd_strategies_trading WHERE user_id = $1)"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let total_pnl: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(pnl), 0) FROM qd_strategy_trades WHERE strategy_id IN (SELECT id FROM qd_strategies_trading WHERE user_id = $1)"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0.0);

    let pending_orders: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_orders WHERE strategy_id IN (SELECT id FROM qd_strategies_trading WHERE user_id = $1) AND status = 'pending'"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let backtest_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM qd_backtest_results WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "strategies": {
            "total": total_strategies,
            "running": running_strategies,
            "stopped": total_strategies - running_strategies,
        },
        "positions": {
            "open": open_positions,
        },
        "trades": {
            "total": total_trades,
            "total_pnl": total_pnl,
        },
        "pending_orders": pending_orders,
        "backtests": backtest_count,
        "exchanges": state.strategy_engine.registered_exchange_names(),
    }))))
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct PositionRow {
    pub id: Uuid,
    pub strategy_id: Uuid,
    pub symbol: String,
    pub side: String,
    pub size: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub leverage: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub opened_at: chrono::DateTime<chrono::Utc>,
    pub closed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct TradeRow {
    pub id: Uuid,
    pub strategy_id: Uuid,
    pub symbol: String,
    pub side: String,
    pub trade_type: String,
    pub price: f64,
    pub amount: f64,
    pub fee: f64,
    pub pnl: f64,
    pub exchange_order_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct PendingOrderRow {
    pub id: Uuid,
    pub strategy_id: Uuid,
    pub symbol: String,
    pub signal_type: String,
    pub order_type: String,
    pub side: String,
    pub amount: f64,
    pub price: Option<f64>,
    pub status: String,
    pub priority: i32,
    pub attempts: i32,
    pub max_attempts: i32,
    pub exchange_order_id: Option<String>,
    pub error_message: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_positions(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let rows = sqlx::query_as::<_, PositionRow>(
        r#"SELECT id, strategy_id, symbol, side, size, entry_price,
           current_price, unrealized_pnl, realized_pnl, leverage,
           stop_loss, take_profit, opened_at, closed_at
           FROM qd_strategy_positions
           WHERE strategy_id IN (SELECT id FROM qd_strategies_trading WHERE user_id = $1)
             AND closed_at IS NULL
           ORDER BY opened_at DESC"#,
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

pub async fn list_trades(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let rows = sqlx::query_as::<_, TradeRow>(
        r#"SELECT id, strategy_id, symbol, side, trade_type, price, amount, fee, pnl,
           exchange_order_id, created_at
           FROM qd_strategy_trades
           WHERE strategy_id IN (SELECT id FROM qd_strategies_trading WHERE user_id = $1)
           ORDER BY created_at DESC LIMIT 100"#,
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

pub async fn list_pending_orders(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let rows = sqlx::query_as::<_, PendingOrderRow>(
        r#"SELECT id, strategy_id, symbol, signal_type,
           order_type, side, amount, price,
           status, priority, attempts, max_attempts,
           exchange_order_id, error_message, created_at, updated_at
           FROM pending_orders
           WHERE strategy_id IN (SELECT id FROM qd_strategies_trading WHERE user_id = $1)
           ORDER BY created_at DESC LIMIT 100"#,
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
