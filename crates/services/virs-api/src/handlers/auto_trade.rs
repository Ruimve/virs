//! Auto trade bot API handlers.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::handlers::auth::{extract_user_id, ApiResponse};
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
    let name = body["name"].as_str().unwrap_or("Auto Bot");

    if symbol.is_empty() || exchange.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err("symbol and exchange are required")),
        ));
    }

    let id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO qd_auto_bots (id, user_id, name, symbol, exchange, market_type, leverage, status, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, 'stopped', NOW(), NOW())"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(name)
    .bind(symbol)
    .bind(exchange)
    .bind(market_type)
    .bind(leverage)
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
    if let Some(ref tx) = state.auto_cmd_tx {
        let _ = tx.send(virs_bot::auto::types::AutoCommand::StartBot { bot_id: id }).await;
    }
    Ok(Json(ApiResponse::ok(serde_json::json!({"started": true}))))
}

pub async fn stop_bot(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    if let Some(ref tx) = state.auto_cmd_tx {
        let _ = tx.send(virs_bot::auto::types::AutoCommand::StopBot { bot_id: id }).await;
    }
    Ok(Json(ApiResponse::ok(serde_json::json!({"stopped": true}))))
}

pub async fn delete_bot(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    if let Some(ref tx) = state.auto_cmd_tx {
        let _ = tx.send(virs_bot::auto::types::AutoCommand::DeleteBot { bot_id: id, close_position: true }).await;
    }
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

    let rows = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, String, String, String, serde_json::Value, Option<String>, String, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT l.id, l.bot_id, l.analysis_type, l.status, l.system_prompt, l.result, l.error, l.user_prompt, l.created_at
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
            "logs": logs.iter().map(|(id, bot_id, analysis_type, status, system_prompt, result, error, user_prompt, created_at)| {
                serde_json::json!({
                    "id": id.to_string(),
                    "bot_id": bot_id.to_string(),
                    "analysis_type": analysis_type,
                    "status": status,
                    "system_prompt": system_prompt,
                    "user_prompt": user_prompt,
                    "result": result,
                    "error": error,
                    "created_at": created_at.to_rfc3339(),
                })
            }).collect::<Vec<_>>()
        }))),
        Err(e) => Json(ApiResponse::err(format!("Database error: {}", e))),
    }
}
