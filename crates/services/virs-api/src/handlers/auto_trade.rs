//! Auto trade bot API handlers.

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
    let market_type = body["market_type"].as_str().unwrap_or("perpetual");
    let leverage = body["leverage"].as_i64().unwrap_or(10) as i32;
    let max_position_pct = body["max_position_pct"].as_f64().unwrap_or(80.0);
    let decide_interval_secs = body["decide_interval_secs"].as_i64().unwrap_or(300) as i32;
    let name = body["name"].as_str().unwrap_or("Auto Bot");
    let paper_mode = body["paper_mode"].as_bool().unwrap_or(true);

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

    // Verify exchange is registered in registry (must be done via /api/credentials/save first)
    let market_type_str = market_type;
    let exchange_key = format!("{}:{}", exchange, market_type_str);
    if state.exchange_registry.get(&exchange_key).is_none() {
        return Err((
            StatusCode::PRECONDITION_FAILED,
            Json(ApiResponse::err("Exchange not registered. Please save API credentials first.")),
        ));
    }

    // Subscribe kline engine for this symbol (backfill + WS push)
    let mt = match market_type_str {
        "spot" => virs_models::MarketType::Spot,
        _ => virs_models::MarketType::Perpetual,
    };
    if let Err(e) = state.kline_engine.subscribe(exchange, symbol, mt).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(format!("Failed to subscribe kline: {}", e))),
        ));
    }

    // Register symbol for paper mode price ticks
    if paper_mode {
        state.engine_manager.register_paper_symbol(exchange.to_string(), symbol.to_string()).await;
    }

    let id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO qd_auto_bots (id, user_id, name, symbol, exchange, market_type, leverage, max_position_pct, decide_interval_secs, paper_mode, status, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'stopped', NOW(), NOW())"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(name)
    .bind(symbol)
    .bind(exchange)
    .bind(market_type)
    .bind(leverage)
    .bind(max_position_pct)
    .bind(decide_interval_secs)
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
        r#"SELECT id, name, symbol, exchange, status, created_at FROM qd_auto_bots WHERE user_id = $1 ORDER BY created_at DESC"#,
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
        String, String, String, String, String, i32, f64, i32,
        chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>,
    )>(
        r#"SELECT name, symbol, exchange, status, market_type, leverage, max_position_pct, decide_interval_secs,
           created_at, updated_at
           FROM qd_auto_bots WHERE id = $1 AND user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
    })?;

    let (name, symbol, exchange, status, market_type, leverage, max_position_pct, decide_interval_secs,
         created_at, updated_at) = match basic {
        Some(b) => b,
        None => return Err((StatusCode::NOT_FOUND, Json(ApiResponse::err("Bot not found")))),
    };

    // Query 2: position & stats
    let pos = sqlx::query_as::<_, (
        Option<String>, f64, f64, f64, f64, f64, Option<f64>,
        Option<String>, Option<String>,
        f64, i32, i32, i32,
    )>(
        r#"SELECT current_side, entry_price, position_size, stop_loss, take_profit, unrealized_pnl, liquidation_price,
           market_regime, ai_analysis,
           total_pnl, total_trades, win_trades, loss_trades
           FROM qd_auto_bots WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
    })?;

    let (current_side, entry_price, position_size, stop_loss, take_profit, unrealized_pnl, liquidation_price,
         market_regime, ai_analysis,
         total_pnl, total_trades, win_trades, loss_trades) = pos;

    // Query 3: recent trades
    let trades_rows = sqlx::query_as::<_, (String, String, f64, f64, f64, f64, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT side, trade_type, price, quantity, pnl, pnl_pct, created_at
           FROM qd_auto_trades WHERE bot_id = $1 ORDER BY created_at DESC LIMIT 50"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    let trades: Vec<serde_json::Value> = trades_rows.iter().map(|(side, trade_type, price, quantity, pnl, pnl_pct, t_created_at)| {
        serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "bot_id": id.to_string(),
            "symbol": &symbol,
            "exchange": &exchange,
            "side": side,
            "trade_type": trade_type,
            "price": price,
            "quantity": quantity,
            "pnl": pnl,
            "pnl_pct": pnl_pct,
            "created_at": t_created_at.to_rfc3339(),
        })
    }).collect();

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "bot": {
            "id": id.to_string(),
            "name": name,
            "symbol": symbol,
            "exchange": exchange,
            "market_type": market_type,
            "status": status,
            "leverage": leverage,
            "max_position_pct": max_position_pct,
            "decide_interval_secs": decide_interval_secs,
            "current_side": current_side,
            "entry_price": entry_price,
            "position_size": position_size,
            "stop_loss": stop_loss,
            "take_profit": take_profit,
            "unrealized_pnl": unrealized_pnl,
            "liquidation_price": liquidation_price,
            "market_regime": market_regime,
            "ai_analysis": ai_analysis,
            "total_pnl": total_pnl,
            "total_trades": total_trades,
            "win_trades": win_trades,
            "loss_trades": loss_trades,
            "created_at": created_at.to_rfc3339(),
            "updated_at": updated_at.to_rfc3339(),
        },
        "trades": trades,
    }))))
}

pub async fn start_bot(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    if let Some(tx) = state.engine_manager.auto_cmd_tx() {
        let _ = tx.send(virs_bot::auto::types::AutoCommand::StartBot { bot_id: id }).await;
    }
    Ok(Json(ApiResponse::ok(serde_json::json!({"started": true}))))
}

pub async fn stop_bot(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    if let Some(tx) = state.engine_manager.auto_cmd_tx() {
        let _ = tx.send(virs_bot::auto::types::AutoCommand::StopBot { bot_id: id }).await;
    }
    Ok(Json(ApiResponse::ok(serde_json::json!({"stopped": true}))))
}

pub async fn delete_bot(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    if let Some(tx) = state.engine_manager.auto_cmd_tx() {
        let _ = tx.send(virs_bot::auto::types::AutoCommand::DeleteBot { bot_id: id, close_position: true }).await;
    }
    // Delete from database
    sqlx::query(r#"DELETE FROM qd_auto_bots WHERE id = $1"#)
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

    let rows = sqlx::query_as::<_, (String, String, f64, f64, f64, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT side, trade_type, price, quantity, pnl, created_at
           FROM qd_auto_trades WHERE bot_id = $1 AND user_id = $2 ORDER BY created_at DESC LIMIT 100"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await;

    match rows {
        Ok(trades) => Json(ApiResponse::ok(serde_json::json!({
            "trades": trades.iter().map(|(side, trade_type, price, quantity, pnl, created_at)| {
                serde_json::json!({
                    "side": side,
                    "type": trade_type,
                    "price": price,
                    "quantity": quantity,
                    "pnl": pnl,
                    "created_at": created_at.to_rfc3339(),
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
           FROM qd_auto_analysis_logs l
           JOIN qd_auto_bots b ON l.bot_id = b.id
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
