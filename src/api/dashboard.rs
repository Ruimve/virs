use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::api::AppState;
use crate::api::middleware::AuthUser;
use crate::models::*;

pub async fn dashboard_summary(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let total_bots: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM qd_grid_bots WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let running_bots: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM qd_grid_bots WHERE user_id = $1 AND status = 'running'"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let total_trades: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM qd_grid_trades WHERE user_id = $1 AND status = 'filled'"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let total_pnl: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(pnl), 0) FROM qd_grid_trades WHERE user_id = $1 AND status = 'filled'"
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0.0);

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "bots": {
            "total": total_bots,
            "running": running_bots,
            "stopped": total_bots - running_bots,
        },
        "trades": {
            "total": total_trades,
            "total_pnl": total_pnl,
        },
        "exchanges": state.exchange_registry.registered_names(),
    }))))
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct GridBotRow {
    pub id: uuid::Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub status: String,
    pub upper_price: f64,
    pub lower_price: f64,
    pub grid_count: i32,
    pub grid_profit_pct: f64,
    pub quantity_per_grid: f64,
    pub leverage: i32,
    pub total_pnl: f64,
    pub total_trades: i32,
    pub grid_filled_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub stopped_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_positions(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let rows = sqlx::query_as::<_, GridBotRow>(
        r#"SELECT id, name, symbol, exchange, status,
           upper_price, lower_price, grid_count, grid_profit_pct,
           quantity_per_grid, leverage, total_pnl, total_trades,
           grid_filled_count, created_at, started_at, stopped_at
           FROM qd_grid_bots
           WHERE user_id = $1
           ORDER BY created_at DESC"#,
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

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct GridTradeRow {
    pub id: uuid::Uuid,
    pub bot_id: uuid::Uuid,
    pub symbol: String,
    pub exchange: String,
    pub grid_level: i32,
    pub open_side: String,
    pub open_price: f64,
    pub open_quantity: f64,
    pub open_order_id: Option<String>,
    pub opened_at: chrono::DateTime<chrono::Utc>,
    pub close_side: Option<String>,
    pub close_price: Option<f64>,
    pub close_quantity: Option<f64>,
    pub close_order_id: Option<String>,
    pub closed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_trades(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;

    let rows = sqlx::query_as::<_, GridTradeRow>(
        r#"SELECT id, bot_id, symbol, exchange, grid_level,
           open_side, open_price, open_quantity, open_order_id, opened_at,
           close_side, close_price, close_quantity, close_order_id, closed_at,
           pnl, pnl_pct, status, created_at
           FROM qd_grid_trades
           WHERE user_id = $1
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
    let _user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;
    let _state = state;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "items": [] }))))
}
