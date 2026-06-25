//! Grid bot API handlers.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

/// 分页查询参数
#[derive(Debug, Deserialize)]
pub struct TradesQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

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

    // Verify exchange is registered in registry (must be done via /api/credentials/save first)
    let exchange_key = format!("{}:{}", exchange, market_type);
    if state.exchange_registry.get(&exchange_key).is_none() {
        return Err((
            StatusCode::PRECONDITION_FAILED,
            Json(ApiResponse::err("Exchange not registered. Please save API credentials first.")),
        ));
    }

    // Subscribe kline engine for this symbol (backfill + WS push)
    let mt = match market_type {
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

    // 从交易所获取真实账户余额，初始化 initial_capital
    // paper 模式 fallback 10000，真实交易 fallback 0（避免误判可用资金导致超额下单）
    let fallback = if paper_mode { 10000.0 } else { 0.0 };
    let initial_capital = match state.exchange_registry.get(&exchange_key) {
        Some(ex) => {
            let quote_asset = extract_quote_asset(symbol);
            match ex.get_balances().await {
                Ok(balances) => balances
                    .iter()
                    .find(|b| b.asset.eq_ignore_ascii_case(&quote_asset))
                    .map(|b| b.total)
                    .unwrap_or_else(|| {
                        tracing::warn!(asset = %quote_asset, paper_mode, fallback, "quote asset not found in balances");
                        fallback
                    }),
                Err(e) => {
                    tracing::warn!(error = %e, paper_mode, fallback, "failed to fetch balances");
                    fallback
                }
            }
        }
        None => fallback,
    };

    let id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO qd_grid_bots (id, user_id, name, symbol, exchange, grid_count, upper_price, lower_price,
           grid_profit_pct, quantity_per_grid, leverage, market_type, paper_mode, initial_capital, status, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 'stopped', NOW(), NOW())"#,
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
    .bind(initial_capital)
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
    })?;

    tracing::info!(bot_id = %id, initial_capital, "Grid bot created with initial_capital from exchange");

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

    let rows = sqlx::query_as::<_, (
        uuid::Uuid, String, String, String, String, String, i32,
        chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>,
    )>(
        r#"SELECT id, name, symbol, exchange, status, market_type, leverage,
           created_at, updated_at
           FROM qd_grid_bots WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await;

    match rows {
        Ok(bots) => {
            let items: Vec<_> = bots.iter().map(|(id, name, symbol, exchange, status, market_type, leverage, created_at, updated_at)| {
                serde_json::json!({
                    "id": id.to_string(),
                    "name": name,
                    "symbol": symbol,
                    "exchange": exchange,
                    "status": status,
                    "market_type": market_type,
                    "leverage": leverage,
                    "created_at": created_at.to_rfc3339(),
                    "updated_at": updated_at.to_rfc3339(),
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
        String, String, String, String, String, f64, f64, i32, f64, f64, i32,
        chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>,
    )>(
        r#"SELECT name, symbol, exchange, status, market_type, upper_price, lower_price,
           grid_count, grid_profit_pct, quantity_per_grid, leverage, created_at, updated_at
           FROM qd_grid_bots WHERE id = $1 AND user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
    })?;

    let (name, symbol, exchange, status, market_type, upper_price, lower_price,
         grid_count, grid_profit_pct, quantity_per_grid, leverage, created_at, updated_at) = match basic {
        Some(b) => b,
        None => return Err((StatusCode::NOT_FOUND, Json(ApiResponse::err("Bot not found")))),
    };

    // Query 2: stats & ai
    let stats = sqlx::query_as::<_, (
        f64, f64, f64, i32, i32, bool,
        Option<String>, Option<String>, Option<serde_json::Value>,
    )>(
        r#"SELECT total_pnl, unrealized_pnl, initial_capital, total_trades, grid_filled_count, dynamic_adjust,
           market_regime, ai_analysis, grid_levels_json
           FROM qd_grid_bots WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(format!("Database error: {}", e))))
    })?;

    let (total_pnl, unrealized_pnl, initial_capital, total_trades, grid_filled_count, dynamic_adjust,
         market_regime, ai_analysis, grid_levels_json) = stats;

    // Parse grid levels from JSON
    let grid_levels: Vec<serde_json::Value> = grid_levels_json
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();

    // Query 3: recent trades
    let trades_rows = sqlx::query_as::<_, (
        uuid::Uuid, i32, String, f64, f64, Option<String>, Option<f64>, Option<f64>, f64, f64, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>,
    )>(
        r#"SELECT id, grid_level, open_side, open_price, open_quantity,
           close_side, close_price, close_quantity, pnl, pnl_pct, status, opened_at, closed_at
           FROM qd_grid_trades WHERE bot_id = $1 ORDER BY opened_at DESC LIMIT 50"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    let trades: Vec<serde_json::Value> = trades_rows.iter().map(|(tid, level, side, open_p, open_qty, close_side, close_p, close_qty, pnl, pnl_pct, t_status, opened_at, closed_at)| {
        serde_json::json!({
            "id": tid.to_string(),
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
            "closed_at": closed_at.map(|t| t.to_rfc3339()),
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
            "grid_count": grid_count,
            "upper_price": upper_price,
            "lower_price": lower_price,
            "grid_profit_pct": grid_profit_pct,
            "quantity_per_grid": quantity_per_grid,
            "initial_capital": initial_capital,
            "total_pnl": total_pnl,
            "unrealized_pnl": unrealized_pnl,
            "total_trades": total_trades,
            "grid_filled_count": grid_filled_count,
            "dynamic_adjust": dynamic_adjust,
            "market_regime": market_regime,
            "ai_analysis": ai_analysis,
            "created_at": created_at.to_rfc3339(),
            "updated_at": updated_at.to_rfc3339(),
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

/// 从交易对符号中提取计价货币（如 "BTC/USDT" → "USDT"，"BTCUSDT" → "USDT"）
fn extract_quote_asset(symbol: &str) -> String {
    if let Some(idx) = symbol.find('/') {
        return symbol[idx + 1..].to_uppercase();
    }
    let upper = symbol.to_uppercase();
    for quote in &["USDT", "USDC", "FDUSD", "BUSD", "TUSD", "BTC", "ETH", "BNB"] {
        if upper.ends_with(quote) {
            return quote.to_string();
        }
    }
    "USDT".to_string()
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
    axum::extract::Query(params): axum::extract::Query<TradesQuery>,
) -> Json<ApiResponse> {
    let user_id = match extract_user_id(&headers) {
        Ok(id) => id,
        Err((_, resp)) => return resp,
    };

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    // 查询总数
    let total: i64 = match sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM qd_grid_trades WHERE bot_id = $1 AND user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    {
        Ok(n) => n,
        Err(e) => return Json(ApiResponse::err(format!("Database error: {}", e))),
    };

    // 查询分页数据（完整字段）
    let rows = sqlx::query_as::<_, (
        uuid::Uuid, i32, String, f64, f64, Option<String>, Option<f64>, Option<f64>, f64, f64, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>,
    )>(
        r#"SELECT id, grid_level, open_side, open_price, open_quantity,
           close_side, close_price, close_quantity, pnl, pnl_pct, status, opened_at, closed_at
           FROM qd_grid_trades WHERE bot_id = $1 AND user_id = $2
           ORDER BY opened_at DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.db_pool)
    .await;

    match rows {
        Ok(trades) => Json(ApiResponse::ok(serde_json::json!({
            "trades": trades.iter().map(|(tid, level, open_side, open_p, open_qty, close_side, close_p, close_qty, pnl, pnl_pct, status, opened_at, closed_at)| {
                serde_json::json!({
                    "id": tid.to_string(),
                    "bot_id": id.to_string(),
                    "grid_level": level,
                    "open_side": open_side,
                    "open_price": open_p,
                    "open_quantity": open_qty,
                    "close_side": close_side,
                    "close_price": close_p,
                    "close_quantity": close_qty,
                    "pnl": pnl,
                    "pnl_pct": pnl_pct,
                    "status": status,
                    "opened_at": opened_at.to_rfc3339(),
                    "closed_at": closed_at.map(|t| t.to_rfc3339()),
                })
            }).collect::<Vec<_>>(),
            "total": total,
            "page": page,
            "page_size": page_size,
        }))),
        Err(e) => Json(ApiResponse::err(format!("Database error: {}", e))),
    }
}

/// 网格机器人统计接口（基于全量 trades 计算统计指标）
pub async fn get_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Json<ApiResponse> {
    let user_id = match extract_user_id(&headers) {
        Ok(id) => id,
        Err((_, resp)) => return resp,
    };

    // 拉取全量已平仓 trades（按时间正序），用于计算统计指标
    let rows = sqlx::query_as::<_, (f64, f64, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT pnl, pnl_pct, opened_at
           FROM qd_grid_trades WHERE bot_id = $1 AND user_id = $2 AND status = 'closed'
           ORDER BY opened_at ASC"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await;

    let trades = match rows {
        Ok(t) => t,
        Err(e) => return Json(ApiResponse::err(format!("Database error: {}", e))),
    };

    // 读取 bot 汇总字段
    let bot_stats = sqlx::query_as::<_, (f64, f64, i32, i32)>(
        r#"SELECT total_pnl, unrealized_pnl, total_trades, grid_filled_count FROM qd_grid_bots WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await;

    let (total_pnl, unrealized_pnl, total_trades, grid_filled_count) = match bot_stats {
        Ok(Some((tp, up, tt, gf))) => (tp, up, tt, gf),
        Ok(None) => (0.0f64, 0.0f64, 0i32, 0i32),
        Err(e) => return Json(ApiResponse::err(format!("Database error: {}", e))),
    };

    // 累计已实现 PnL（从 trades 重新计算，确保一致性）
    let realized_pnl: f64 = trades.iter().map(|t| t.0).sum();
    let net_pnl = realized_pnl + unrealized_pnl;

    // 胜率（基于已平仓 trades）
    let win_trades = trades.iter().filter(|t| t.0 > 0.0).count() as i32;
    let loss_trades = trades.iter().filter(|t| t.0 < 0.0).count() as i32;
    let closed_count = trades.len() as i32;
    let win_rate = if closed_count > 0 {
        (win_trades as f64 / closed_count as f64) * 100.0
    } else {
        0.0
    };

    // 盈亏比 = 平均盈利 / 平均亏损
    let profits: Vec<f64> = trades.iter().filter(|t| t.0 > 0.0).map(|t| t.0).collect();
    let losses: Vec<f64> = trades.iter().filter(|t| t.0 < 0.0).map(|t| t.0).collect();
    let avg_profit = if !profits.is_empty() { profits.iter().sum::<f64>() / profits.len() as f64 } else { 0.0 };
    let avg_loss = if !losses.is_empty() { losses.iter().sum::<f64>() / losses.len() as f64 } else { 0.0 };
    let profit_loss_ratio = if avg_loss.abs() > 0.0 {
        avg_profit / avg_loss.abs()
    } else if avg_profit > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };

    // 最大回撤（基于累计 PnL 峰值）
    let mut cumulative = 0.0f64;
    let mut peak = 0.0f64;
    let mut max_drawdown = 0.0f64;
    for t in &trades {
        cumulative += t.0;
        if cumulative > peak {
            peak = cumulative;
        }
        let drawdown = peak - cumulative;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }

    // 平均持仓时间（开仓到平仓的时间差）— grid_trades 无 closed_at 字段，使用 opened_at 间隔估算
    let avg_hold_time = if trades.len() > 1 {
        let mut total_diff_ms = 0i64;
        let mut count = 0i64;
        for i in 1..trades.len() {
            let diff = trades[i].2.timestamp_millis() - trades[i - 1].2.timestamp_millis();
            if diff > 0 {
                total_diff_ms += diff;
                count += 1;
            }
        }
        if count > 0 {
            format_duration(total_diff_ms / count)
        } else {
            "-".to_string()
        }
    } else {
        "-".to_string()
    };

    // 连胜/连亏
    let mut max_win_streak = 0i32;
    let mut max_loss_streak = 0i32;
    let mut current_win = 0i32;
    let mut current_loss = 0i32;
    for t in &trades {
        if t.0 > 0.0 {
            current_win += 1;
            current_loss = 0;
            if current_win > max_win_streak {
                max_win_streak = current_win;
            }
        } else if t.0 < 0.0 {
            current_loss += 1;
            current_win = 0;
            if current_loss > max_loss_streak {
                max_loss_streak = current_loss;
            }
        }
    }

    // 平均盈亏（每笔交易）
    let avg_pnl = if !trades.is_empty() { realized_pnl / trades.len() as f64 } else { 0.0 };

    // 最大单笔盈利 / 亏损
    let max_profit: f64 = trades.iter().map(|t| t.0).fold(0.0f64, |a, b| a.max(b));
    let max_loss: f64 = trades.iter().map(|t| t.0).fold(0.0f64, |a, b| a.min(b));

    Json(ApiResponse::ok(serde_json::json!({
        "win_rate": win_rate,
        "profit_loss_ratio": profit_loss_ratio,
        "max_drawdown": max_drawdown,
        "avg_hold_time": avg_hold_time,
        "max_win_streak": max_win_streak,
        "max_loss_streak": max_loss_streak,
        "net_pnl": net_pnl,
        "realized_pnl": realized_pnl,
        "unrealized_pnl": unrealized_pnl,
        "total_pnl": total_pnl,
        "total_trades": total_trades,
        "closed_trades": closed_count,
        "win_trades": win_trades,
        "loss_trades": loss_trades,
        "grid_filled_count": grid_filled_count,
        "avg_pnl": avg_pnl,
        "max_profit": max_profit,
        "max_loss": max_loss,
    })))
}

fn format_duration(ms: i64) -> String {
    if ms <= 0 {
        return "-".to_string();
    }
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    if hours > 0 {
        format!("{}h{}m", hours, minutes % 60)
    } else if minutes > 0 {
        format!("{}m{}s", minutes, seconds % 60)
    } else {
        format!("{}s", seconds)
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
