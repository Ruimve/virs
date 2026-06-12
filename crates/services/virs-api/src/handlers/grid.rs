//! Grid bot API handlers.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

pub async fn create_bot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let user_id = extract_user_id(&headers)?;

    let symbol = body["symbol"].as_str().unwrap_or("");
    let exchange = body["exchange"].as_str().unwrap_or("");
    let grid_count = body["grid_count"].as_i64().unwrap_or(10) as i32;
    let upper_price = body["upper_price"].as_f64().unwrap_or(0.0);
    let lower_price = body["lower_price"].as_f64().unwrap_or(0.0);
    let grid_profit_pct = body["grid_profit_pct"].as_f64().unwrap_or(0.5);
    let quantity_per_grid = body["quantity_per_grid"].as_f64().unwrap_or(10.0);
    let leverage = body["leverage"].as_i64().unwrap_or(5) as i32;
    let name = body["name"].as_str().unwrap_or("Grid Bot");
    let paper_mode = body["paper_mode"].as_bool().unwrap_or(true);
    let market_type = body["market_type"].as_str().unwrap_or("perpetual");

    if symbol.is_empty() || exchange.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err("symbol and exchange are required")),
        ));
    }

    // Enforce 1-bot-per-user limit (across all bot types)
    {
        let grid_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM qd_grid_bots WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);
        let auto_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM qd_auto_bots WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);
        if grid_count + auto_count > 0 {
            return Err((
                StatusCode::CONFLICT,
                Json(ApiResponse::err("Each account can only have one bot. Please delete your existing bot first.")),
            ));
        }
    }

    // Ensure engines are started (lazy init on first bot creation)
    if let Err(e) = state.engine_manager.ensure_started(paper_mode).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(format!("Failed to start engines: {}", e))),
        ));
    }

    let id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO qd_grid_bots (id, user_id, name, symbol, exchange, grid_count, upper_price, lower_price,
           grid_profit_pct, quantity_per_grid, leverage, market_type, paper_mode, status, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'stopped', NOW(), NOW())"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(name)
    .bind(symbol)
    .bind(exchange)
    .bind(grid_count)
    .bind(upper_price)
    .bind(lower_price)
    .bind(grid_profit_pct)
    .bind(quantity_per_grid)
    .bind(leverage)
    .bind(market_type)
    .bind(paper_mode)
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
    })?;

    Ok(Json(ApiResponse::ok(serde_json::json!({"id": id.to_string()}))))
}

pub async fn list_bots(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<ApiResponse> {
    let user_id = match extract_user_id(&headers) {
        Ok(id) => id,
        Err((_, resp)) => return resp,
    };

    let rows = sqlx::query_as::<_, (uuid::Uuid, String, String, String, String, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT id, name, symbol, exchange, status, created_at FROM qd_grid_bots WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await;

    match rows {
        Ok(bots) => {
            let items: Vec<_> = bots.iter().map(|(id, name, symbol, exchange, status, created_at)| {
                serde_json::json!({
                    "id": id.to_string(),
                    "name": name,
                    "symbol": symbol,
                    "exchange": exchange,
                    "status": status,
                    "created_at": created_at.to_rfc3339(),
                })
            }).collect();
            let total = items.len();
            Json(ApiResponse::ok(serde_json::json!({
                "items": items,
                "total": total,
            })))
        }
        Err(e) => Json(ApiResponse::err(format!("Database error: {}", e))),
    }
}

pub async fn get_bot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let user_id = extract_user_id(&headers)?;

    // Query 1: basic info
    let basic = sqlx::query_as::<_, (
        String, String, String, String, f64, f64, i32, f64, f64, i32,
        chrono::DateTime<chrono::Utc>,
    )>(
        r#"SELECT name, symbol, exchange, status, upper_price, lower_price,
           grid_count, grid_profit_pct, quantity_per_grid, leverage, created_at
           FROM qd_grid_bots WHERE id = $1 AND user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
    })?;

    let (name, symbol, exchange, status, upper_price, lower_price,
         grid_count, grid_profit_pct, quantity_per_grid, leverage, created_at) = match basic {
        Some(b) => b,
        None => return Err((StatusCode::NOT_FOUND, Json(ApiResponse::err("Bot not found")))),
    };

    // Query 2: stats & ai
    let stats = sqlx::query_as::<_, (
        f64, f64, i32, i32, bool,
        Option<String>, Option<String>, Option<serde_json::Value>,
    )>(
        r#"SELECT total_pnl, unrealized_pnl, total_trades, grid_filled_count, dynamic_adjust,
           market_regime, ai_analysis, grid_levels_json
           FROM qd_grid_bots WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
    })?;

    let (total_pnl, unrealized_pnl, total_trades, grid_filled_count, dynamic_adjust,
         market_regime, ai_analysis, grid_levels_json) = stats;

    // Parse grid levels from JSON
    let grid_levels: Vec<serde_json::Value> = grid_levels_json
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();

    // Query 3: recent trades
    let trades_rows = sqlx::query_as::<_, (
        i32, String, f64, f64, Option<String>, Option<f64>, Option<f64>, f64, f64, String, chrono::DateTime<chrono::Utc>,
    )>(
        r#"SELECT grid_level, open_side, open_price, open_quantity,
           close_side, close_price, close_quantity, pnl, pnl_pct, status, opened_at
           FROM qd_grid_trades WHERE bot_id = $1 ORDER BY opened_at DESC LIMIT 50"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    let trades: Vec<serde_json::Value> = trades_rows.iter().map(|(level, side, open_p, open_qty, close_side, close_p, close_qty, pnl, pnl_pct, t_status, opened_at)| {
        serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "bot_id": id.to_string(),
            "grid_level": level,
            "open_side": side,
            "open_price": open_p,
            "open_quantity": open_qty,
            "close_side": close_side,
            "close_price": close_p,
            "close_quantity": close_qty,
            "pnl": pnl,
            "pnl_pct": pnl_pct,
            "status": t_status,
            "opened_at": opened_at.to_rfc3339(),
        })
    }).collect();

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "bot": {
            "id": id.to_string(),
            "name": name,
            "symbol": symbol,
            "exchange": exchange,
            "status": status,
            "leverage": leverage,
            "grid_count": grid_count,
            "upper_price": upper_price,
            "lower_price": lower_price,
            "grid_profit_pct": grid_profit_pct,
            "quantity_per_grid": quantity_per_grid,
            "total_pnl": total_pnl,
            "unrealized_pnl": unrealized_pnl,
            "total_trades": total_trades,
            "grid_filled_count": grid_filled_count,
            "dynamic_adjust": dynamic_adjust,
            "market_regime": market_regime,
            "ai_analysis": ai_analysis,
            "created_at": created_at.to_rfc3339(),
        },
        "trades": trades,
        "grid_levels": grid_levels,
    }))))
}

pub async fn start_bot(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    if let Some(tx) = state.engine_manager.grid_cmd_tx() {
        let _ = tx.send(virs_bot::grid::types::GridCommand::StartBot { bot_id: id }).await;
    }
    Ok(Json(ApiResponse::ok(serde_json::json!({"started": true}))))
}

pub async fn stop_bot(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    if let Some(tx) = state.engine_manager.grid_cmd_tx() {
        let _ = tx.send(virs_bot::grid::types::GridCommand::StopBot { bot_id: id }).await;
    }
    Ok(Json(ApiResponse::ok(serde_json::json!({"stopped": true}))))
}

pub async fn delete_bot(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    if let Some(tx) = state.engine_manager.grid_cmd_tx() {
        let _ = tx.send(virs_bot::grid::types::GridCommand::DeleteBot { bot_id: id, close_position: true }).await;
    }
    // Delete from database
    sqlx::query(r#"DELETE FROM qd_grid_bots WHERE id = $1"#)
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e)))))?;
    Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true}))))
}

pub async fn get_trades(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Json<ApiResponse> {
    let user_id = match extract_user_id(&headers) {
        Ok(id) => id,
        Err((_, resp)) => return resp,
    };

    let rows = sqlx::query_as::<_, (i32, String, f64, Option<f64>, f64, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT grid_level, open_side, open_price, close_price, pnl, opened_at
           FROM qd_grid_trades WHERE bot_id = $1 AND user_id = $2 ORDER BY opened_at DESC LIMIT 100"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await;

    match rows {
        Ok(trades) => Json(ApiResponse::ok(serde_json::json!({
            "trades": trades.iter().map(|(level, side, open_p, close_p, pnl, opened_at)| {
                serde_json::json!({
                    "grid_level": level,
                    "side": side,
                    "open_price": open_p,
                    "close_price": close_p,
                    "pnl": pnl,
                    "opened_at": opened_at.to_rfc3339(),
                })
            }).collect::<Vec<_>>()
        }))),
        Err(e) => Json(ApiResponse::err(format!("Database error: {}", e))),
    }
}

pub async fn get_analysis_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<ApiResponse> {
    let user_id = match extract_user_id(&headers) {
        Ok(id) => id,
        Err((_, resp)) => return resp,
    };

    let rows = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, String, String, String, serde_json::Value, Option<String>, String, String, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT l.id, l.bot_id, l.analysis_type, l.status, l.system_prompt, l.result, l.error, l.user_prompt, l.llm_model, l.created_at
           FROM qd_grid_analysis_logs l
           JOIN qd_grid_bots b ON l.bot_id = b.id
           WHERE b.user_id = $1
           ORDER BY l.created_at DESC LIMIT 50"#,
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await;

    match rows {
        Ok(logs) => Json(ApiResponse::ok(serde_json::json!({
            "logs": logs.iter().map(|(id, bot_id, analysis_type, status, system_prompt, result, error, user_prompt, llm_model, created_at)| {
                serde_json::json!({
                    "id": id.to_string(),
                    "bot_id": bot_id.to_string(),
                    "analysis_type": analysis_type,
                    "status": status,
                    "system_prompt": system_prompt,
                    "user_prompt": user_prompt,
                    "result": result,
                    "error": error,
                    "llm_model": llm_model,
                    "created_at": created_at.to_rfc3339(),
                })
            }).collect::<Vec<_>>()
        }))),
        Err(e) => Json(ApiResponse::err(format!("Database error: {}", e))),
    }
}
