use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::middleware::AuthUser;
use crate::api::AppState;
use crate::models::*;

// ── Request / Response Types ──

#[derive(Debug, Deserialize)]
pub struct CreateBotRequest {
    pub name: String,
    pub symbol: String,
    pub exchange: Option<String>,
    pub grid_count: Option<i32>,
    pub grid_profit_pct: Option<f64>,
    pub quantity_per_grid: Option<f64>,
    pub leverage: Option<i32>,
    pub dynamic_adjust: Option<bool>,
    pub adjust_interval_secs: Option<i32>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
}

// ── Helpers ──

fn parse_user_id(auth: &AuthUser) -> Result<Uuid, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    Uuid::parse_str(&auth.user_id).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<serde_json::Value>::err("Invalid user identity")),
        )
    })
}
// ── 3.2 POST /api/grid/create ──

pub async fn create_bot(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<CreateBotRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    if body.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("name must not be empty")),
        ));
    }

    if body.symbol.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("symbol must not be empty")),
        ));
    }

    let symbol = crate::api::normalize_symbol(&body.symbol);

    let user_id = parse_user_id(&auth)?;

    let upper_price = 0.0;
    let lower_price = 0.0;

    let grid_count = body.grid_count.unwrap_or(0);
    let grid_profit_pct = body.grid_profit_pct.unwrap_or(0.5);
    let quantity_per_grid = body.quantity_per_grid.unwrap_or(10.0);
    let leverage = body.leverage.unwrap_or(1);
    let exchange = body.exchange.unwrap_or_else(|| "binance".to_string());
    let dynamic_adjust = body.dynamic_adjust.unwrap_or(true);
    let adjust_interval_secs = body.adjust_interval_secs.unwrap_or(300);

    let row = sqlx::query_as::<_, GridBot>(
        r#"INSERT INTO qd_grid_bots (
            user_id, name, symbol, exchange, status,
            upper_price, lower_price, grid_count, grid_profit_pct, quantity_per_grid, leverage,
            market_regime, ai_analysis, grid_levels_json, system_prompt, user_prompt,
            dynamic_adjust, adjust_interval_secs
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14::jsonb, $15, $16, $17, $18)
        RETURNING *"#,
    )
    .bind(user_id)
    .bind(&body.name)
    .bind(&symbol)
    .bind(&exchange)
    .bind(StrategyStatus::Draft)
    .bind(upper_price)
    .bind(lower_price)
    .bind(grid_count)
    .bind(grid_profit_pct)
    .bind(quantity_per_grid)
    .bind(leverage)
    .bind(&None::<String>)
    .bind(&None::<String>)
    .bind(&None::<serde_json::Value>)
    .bind(&body.system_prompt)
    .bind(&body.user_prompt)
    .bind(dynamic_adjust)
    .bind(adjust_interval_secs)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to create grid bot: {}", e
            ))),
        )
    })?;

    Ok(Json(ApiResponse::ok(serde_json::json!({ "bot": row }))))
}

// ── 3.3 GET /api/grid/list ──

pub async fn list_bots(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let user_id = parse_user_id(&auth)?;
    let (page, page_size) = params.normalize();
    let offset = (page - 1) * page_size;

    let total: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM qd_grid_bots WHERE user_id = $1"#,
    )
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let bots = sqlx::query_as::<_, GridBot>(
        r#"SELECT * FROM qd_grid_bots WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(user_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let total_pages = (total.0 + page_size - 1) / page_size;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": bots,
        "total": total.0,
        "page": page,
        "page_size": page_size,
        "total_pages": total_pages,
    }))))
}

// ── Grid Levels Builder ──

fn build_grid_levels(bot: &GridBot) -> Vec<serde_json::Value> {
    if bot.grid_count <= 0 || bot.upper_price <= 0.0 || bot.lower_price <= 0.0 || bot.upper_price <= bot.lower_price {
        return vec![];
    }

    let snapshot_levels: Vec<serde_json::Value> = bot.grid_levels_json
        .as_ref()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();

    let grid_spacing = (bot.upper_price - bot.lower_price) / bot.grid_count as f64;
    let profit_factor = 1.0 + bot.grid_profit_pct / 100.0;
    let mid_price = (bot.upper_price + bot.lower_price) / 2.0;

    (0..bot.grid_count)
        .map(|i| {
            let price = bot.lower_price + grid_spacing * (i as f64 + 0.5);
            let snap = snapshot_levels.iter().find(|v| v["level"].as_i64() == Some(i as i64));

            let side = snap
                .and_then(|l| l["side"].as_str())
                .unwrap_or(if price < mid_price { "buy" } else { "sell" });

            let (buy_price, sell_price) = if let Some(l) = snap {
                let bp = l["buy_price"].as_f64().unwrap_or_else(|| if side == "buy" { price } else { price / profit_factor });
                let sp = l["sell_price"].as_f64().unwrap_or_else(|| if side == "buy" { price * profit_factor } else { price });
                (bp, sp)
            } else if side == "buy" {
                (price, price * profit_factor)
            } else {
                (price / profit_factor, price)
            };

            let quantity = snap.and_then(|v| v["quantity"].as_f64())
                .unwrap_or_else(|| bot.quantity_per_grid / price);

            let hold_quantity = snap.and_then(|v| v["hold_quantity"].as_f64()).unwrap_or(0.0);
            let avg_buy_price = snap.and_then(|v| v["avg_buy_price"].as_f64()).unwrap_or(0.0);
            let last_fill_price = snap.and_then(|v| v["last_fill_price"].as_f64()).unwrap_or(0.0);
            let buy_filled = snap.and_then(|v| v["buy_filled"].as_bool()).unwrap_or(false);
            let sell_filled = snap.and_then(|v| v["sell_filled"].as_bool()).unwrap_or(false);
            let filled = hold_quantity.abs() > 0.0 || (buy_filled && sell_filled);

            serde_json::json!({
                "level": i,
                "price": price,
                "side": side,
                "buy_price": buy_price,
                "sell_price": sell_price,
                "open_price": if side == "buy" { buy_price } else { sell_price },
                "close_price": if side == "buy" { sell_price } else { buy_price },
                "filled": filled,
                "quantity": quantity,
                "buy_filled": buy_filled,
                "sell_filled": sell_filled,
                "hold_quantity": hold_quantity,
                "avg_buy_price": avg_buy_price,
                "last_fill_price": last_fill_price,
            })
        })
        .collect()
}

// ── 3.4 GET /api/grid/{id} ──

pub async fn get_bot(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let user_id = parse_user_id(&auth)?;

    let bot = sqlx::query_as::<_, GridBot>(
        r#"SELECT * FROM qd_grid_bots WHERE id = $1 AND user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let bot = match bot {
        Some(b) => b,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<serde_json::Value>::err("Grid bot not found")),
            ));
        }
    };

    let trades = sqlx::query_as::<_, GridTrade>(
        r#"SELECT * FROM qd_grid_trades WHERE bot_id = $1 ORDER BY created_at DESC LIMIT 50"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let grid_levels = build_grid_levels(&bot);

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "bot": bot,
        "trades": trades,
        "grid_levels": grid_levels,
    }))))
}

// ── 3.5 POST /api/grid/{id}/start ──

pub async fn start_bot(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let user_id = parse_user_id(&auth)?;

    let bot = sqlx::query_as::<_, GridBot>(
        r#"SELECT * FROM qd_grid_bots WHERE id = $1 AND user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let bot = match bot {
        Some(b) => b,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<serde_json::Value>::err("Grid bot not found")),
            ));
        }
    };


    match bot.status {
        StrategyStatus::Running => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err("Bot is already running")),
            ));
        }
        _ => {}
    }

    if bot.upper_price <= 0.0 || bot.lower_price <= 0.0 || bot.grid_count <= 0 {
        if !bot.dynamic_adjust {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(
                    "Bot has no valid parameters and dynamic_adjust is disabled. Use /analyze first or enable dynamic_adjust.",
                )),
            ));
        }
        tracing::info!(bot_id = %id, "Bot has no valid parameters, Worker will trigger initial LLM analysis");
    }

    let _ = super::market::ensure_exchange(&state, &bot.exchange, MarketType::Perpetual).await;

    // 通过 GridEngine 启动 bot（首次分析由 Worker 内部自动触发）
    // Engine 负责更新 DB 状态，避免 API 与 Engine 双写竞态
    if let Some(ref grid_cmd_tx) = state.grid_cmd_tx {
        if let Err(e) = grid_cmd_tx.send(crate::bot::semi_automatic_grid::types::GridCommand::StartBot { bot_id: id }).await {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Failed to start grid engine: {}", e))),
            ));
        }
    }

    Ok(Json(ApiResponse::ok_with_message(
        serde_json::json!({ "bot_id": id }),
        "Start command sent, bot will transition to running state",
    )))
}

// ── 3.6 POST /api/grid/{id}/stop ──

pub async fn stop_bot(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let user_id = parse_user_id(&auth)?;

    let bot = sqlx::query_as::<_, GridBot>(
        r#"SELECT * FROM qd_grid_bots WHERE id = $1 AND user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let bot = match bot {
        Some(b) => b,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<serde_json::Value>::err("Grid bot not found")),
            ));
        }
    };

    if bot.status != StrategyStatus::Running && bot.status != StrategyStatus::Paused {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("Bot is not running or paused")),
        ));
    }

    // 通过 GridEngine 停止 bot
    // Engine 负责更新 DB 状态，避免 API 与 Engine 双写竞态
    if let Some(ref grid_cmd_tx) = state.grid_cmd_tx {
        if let Err(e) = grid_cmd_tx.send(crate::bot::semi_automatic_grid::types::GridCommand::StopBot { bot_id: id }).await {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Failed to stop grid engine: {}", e))),
            ));
        }
    }

    Ok(Json(ApiResponse::ok_with_message(
        serde_json::json!({ "bot_id": id }),
        "Stop command sent, bot will transition to stopped state",
    )))
}

// ── 3.7 DELETE /api/grid/{id}/delete ──

#[derive(Debug, Deserialize)]
pub struct DeleteBotParams {
    pub close_position: Option<bool>,
}

pub async fn delete_bot(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(params): Query<DeleteBotParams>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let user_id = parse_user_id(&auth)?;
    let close_position = params.close_position.unwrap_or(false);

    let bot = sqlx::query_as::<_, GridBot>(
        r#"SELECT * FROM qd_grid_bots WHERE id = $1 AND user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    match bot {
        Some(b) => {
            if b.status == StrategyStatus::Running && !close_position {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<serde_json::Value>::err(
                        "Cannot delete a running bot without close_position. Use close_position=true to stop and close positions.",
                    )),
                ));
            }

            if let Some(ref grid_cmd_tx) = state.grid_cmd_tx {
                if let Err(e) = grid_cmd_tx.send(crate::bot::semi_automatic_grid::types::GridCommand::DeleteBot {
                    bot_id: id,
                    close_position,
                }).await {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiResponse::<serde_json::Value>::err(format!(
                            "Failed to send delete command: {}", e
                        ))),
                    ));
                }
            } else {
                sqlx::query("DELETE FROM qd_grid_bots WHERE id = $1")
                    .bind(id)
                    .execute(&state.db_pool)
                    .await
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ApiResponse::<serde_json::Value>::err(format!(
                                "Failed to delete bot: {}", e
                            ))),
                        )
                    })?;
            }
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<serde_json::Value>::err("Grid bot not found")),
            ));
        }
    }

    Ok(Json(ApiResponse::ok_with_message(
        serde_json::json!({ "deleted": true, "close_position": close_position }),
        if close_position {
            "Grid bot deleted with positions closed"
        } else {
            "Grid bot deleted (positions remain open)"
        },
    )))
}

// ── 3.8 GET /api/grid/{id}/trades ──

pub async fn get_trades(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let user_id = parse_user_id(&auth)?;
    let (page, page_size) = params.normalize();
    let offset = (page - 1) * page_size;

    let _bot: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT id FROM qd_grid_bots WHERE id = $1 AND user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    if _bot.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("Grid bot not found")),
        ));
    }

    let total: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM qd_grid_trades WHERE bot_id = $1"#,
    )
    .bind(id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let trades = sqlx::query_as::<_, GridTrade>(
        r#"SELECT * FROM qd_grid_trades WHERE bot_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let bot = sqlx::query_as::<_, GridBot>(
        r#"SELECT * FROM qd_grid_bots WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .ok()
    .flatten();

    let grid_levels = bot.as_ref().map(|b| build_grid_levels(b)).unwrap_or_default();

    let total_pages = (total.0 + page_size - 1) / page_size;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": trades,
        "total": total.0,
        "page": page,
        "page_size": page_size,
        "total_pages": total_pages,
        "grid_levels": grid_levels,
    }))))
}


// ── Paper Trading ──

/// GET /api/grid/paper/status — 获取 paper 交易状态
pub async fn paper_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "enabled": state.paper_mode,
    }))))
}

/// POST /api/grid/paper/enable — 启用 paper 交易
///
/// Paper 模式由配置决定（PAPER_TRADING 环境变量），运行时不可切换。
/// 此端点返回当前状态提示。
pub async fn paper_enable(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    if state.paper_mode {
        Ok(Json(ApiResponse::ok_with_message(
            serde_json::json!({ "enabled": true }),
            "Paper trading is already enabled (controlled by PAPER_TRADING config)",
        )))
    } else {
        Err((
            StatusCode::CONFLICT,
            Json(ApiResponse::<serde_json::Value>::err(
                "Paper mode is controlled by PAPER_TRADING environment variable. Restart with PAPER_TRADING=true to enable.",
            )),
        ))
    }
}

/// POST /api/grid/paper/disable — 禁用 paper 交易
///
/// Paper 模式由配置决定（PAPER_TRADING 环境变量），运行时不可切换。
/// 此端点返回当前状态提示。
pub async fn paper_disable(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    if !state.paper_mode {
        Ok(Json(ApiResponse::ok_with_message(
            serde_json::json!({ "enabled": false }),
            "Paper trading is already disabled (real exchange mode)",
        )))
    } else {
        Err((
            StatusCode::CONFLICT,
            Json(ApiResponse::<serde_json::Value>::err(
                "Paper mode is controlled by PAPER_TRADING environment variable. Restart with PAPER_TRADING=false to disable.",
            )),
        ))
    }
}

/// GET /api/grid/analysis-logs?bot_id=xxx — 获取分析日志
pub async fn get_analysis_logs(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let user_id = parse_user_id(&auth)?;

    let bot_id = match params.get("bot_id").and_then(|s| s.parse::<Uuid>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err("bot_id query parameter is required")),
            ));
        }
    };

    // 验证 bot 属于当前用户
    let bot_exists: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT id FROM qd_grid_bots WHERE id = $1 AND user_id = $2"#,
    )
    .bind(bot_id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    if bot_exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("Grid bot not found")),
        ));
    }

    let logs = sqlx::query_as::<_, (Uuid, Uuid, String, String, String, String, serde_json::Value, Option<String>, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"SELECT id, bot_id, analysis_type, status, system_prompt, user_prompt, result, error, created_at, completed_at
           FROM qd_grid_analysis_logs WHERE bot_id = $1 ORDER BY created_at DESC LIMIT 50"#,
    )
    .bind(bot_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    let items: Vec<serde_json::Value> = logs.into_iter().map(|r| {
        serde_json::json!({
            "id": r.0,
            "bot_id": r.1,
            "analysis_type": r.2,
            "status": r.3,
            "system_prompt": r.4,
            "user_prompt": r.5,
            "result": r.6,
            "error": r.7,
            "created_at": r.8,
            "completed_at": r.9,
        })
    }).collect();

    Ok(Json(ApiResponse::ok(serde_json::json!({ "items": items }))))
}
