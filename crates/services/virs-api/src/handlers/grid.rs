use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use virs_strategy::prompt::StrategyType;
use virs_error::VirsError;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::handlers::strategy_selection::select_strategy_by_llm;
use crate::handlers::utils::{format_duration, TradesQuery};
use crate::state::AppState;


pub async fn create_bot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let symbol = body["symbol"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("symbol is required"))?;
    let exchange = body["exchange"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("exchange is required"))?;
    let grid_count = body["grid_count"].as_i64().ok_or_else(|| {
        VirsError::bad_request("grid_count is required and must be greater than 0")
    })? as i32;
    let upper_price = body["upper_price"].as_f64().ok_or_else(|| {
        VirsError::bad_request("upper_price is required and must be greater than 0")
    })?;
    let lower_price = body["lower_price"].as_f64().ok_or_else(|| {
        VirsError::bad_request("lower_price is required and must be greater than 0")
    })?;
    let grid_profit_pct = body["grid_profit_pct"].as_f64().ok_or_else(|| {
        VirsError::bad_request("grid_profit_pct is required and must be greater than 0")
    })?;
    let quantity_per_grid = body["quantity_per_grid"].as_f64().ok_or_else(|| {
        VirsError::bad_request("quantity_per_grid is required and must be greater than 0")
    })?;
    let leverage = body["leverage"].as_i64().ok_or_else(|| {
        VirsError::bad_request("leverage is required and must be greater than 0")
    })? as i32;
    let name = body["name"].as_str().unwrap_or("Grid Bot");
    let paper_mode = body["paper_mode"].as_bool().ok_or_else(|| {
        VirsError::bad_request("paper_mode is required (must be true or false)")
    })?;

    if upper_price <= lower_price {
        return Err(VirsError::bad_request(
            "upper_price must be greater than lower_price",
        ));
    }
    if grid_count <= 0 {
        return Err(VirsError::bad_request(
            "grid_count must be greater than 0",
        ));
    }
    if grid_profit_pct <= 0.0 {
        return Err(VirsError::bad_request(
            "grid_profit_pct must be greater than 0",
        ));
    }
    if leverage <= 0 {
        return Err(VirsError::bad_request(
            "leverage is required and must be greater than 0",
        ));
    }


    {
        let grid_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM qd_grid_bots WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&state.db_pool)
                .await?;
        let auto_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM qd_auto_bots WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&state.db_pool)
                .await?;
        if grid_count + auto_count > 0 {
            return Err(VirsError::conflict(
                "Each account can only have one bot. Please delete your existing bot first.",
            ));
        }
    }


    state.engine_manager.ensure_started(paper_mode).await?;


    let exchange_key = format!("{}:{}", exchange, virs_types::MarketType::Perpetual);
    if state.exchange_registry.get(&exchange_key).is_none() {
        return Err(VirsError::Http {
            status: 412,
            message: "Exchange not registered. Please save API credentials first.".into(),
        });
    }


    state.kline_engine.subscribe(exchange, symbol, virs_types::MarketType::Perpetual).await?;


    if paper_mode {
        state
            .engine_manager
            .register_paper_symbol(exchange.to_string(), symbol.to_string())
            .await;
    }


    let fallback = if paper_mode { 10000.0 } else { 0.0 };
    let initial_capital = match state.exchange_registry.get(&exchange_key) {
        Some(ex) => {
            // ExchangePe::get_balance() 直接返回单个（通常为 USDT）余额，不再返回多币种 Vec
            let quote_asset = extract_quote_asset(symbol);
            match ex.get_balance().await {
                Ok(b) => {
                    if !b.asset.eq_ignore_ascii_case(&quote_asset) {
                        tracing::warn!(asset = %quote_asset, balance_asset = %b.asset, paper_mode, fallback, "get_balance asset != quote asset; using balance total");
                    }
                    if b.total > 0.0 {
                        b.total
                    } else {
                        tracing::warn!(paper_mode, fallback, "balance total is zero; using fallback");
                        fallback
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, paper_mode, fallback, "failed to fetch balance");
                    fallback
                }
            }
        }
        None => fallback,
    };

    let id = uuid::Uuid::new_v4();

    // 策略选择：从全局 PromptLoader 获取 grid 策略列表
    let loader = state.prompt_loader.clone();
    let strategies = loader.list(StrategyType::Grid).await;

    let strategy_file = match strategies.len() {
        0 => return Err(VirsError::bad_request(
            "No grid strategy available. Please create a strategy first.",
        )),
        1 => strategies[0].clone(),
        _ => {
            // 多策略：LLM 分析市场数据后选择
            select_strategy_by_llm(&state, &loader, &strategies, exchange, symbol, StrategyType::Grid).await?
        }
    };

    // 校验策略在 loader 中存在
    if loader.get(StrategyType::Grid, &strategy_file).await.is_none() {
        return Err(VirsError::bad_request(
            format!("Strategy '{strategy_file}' not found in loaded strategies"),
        ));
    }

    sqlx::query(
        r#"INSERT INTO qd_grid_bots (id, user_id, name, symbol, exchange, grid_count, upper_price, lower_price,
           grid_profit_pct, quantity_per_grid, leverage, paper_mode, initial_capital, status, strategy_file, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'stopped', $14, NOW(), NOW())"#,
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
    .bind(paper_mode)
    .bind(initial_capital)
    .bind(&strategy_file)
    .execute(&state.db_pool)
    .await?;

    // 如果是 LLM 选择的策略，记录选择日志
    if strategies.len() > 1 {
        let _ = sqlx::query(
            r#"INSERT INTO qd_grid_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status, result, strategy_file, completed_at)
               VALUES ($1, 'strategy_selection', $2, $3, 'completed', $4, $5, NOW())"#,
        )
        .bind(id)
        .bind("You are a trading strategy selector. Choose the best strategy for the current market conditions.")
        .bind(format!("Symbol: {}, Exchange: {}, Strategies: {:?}", symbol, exchange, strategies))
        .bind(serde_json::json!({"selected": strategy_file}))
        .bind(&strategy_file)
        .execute(&state.db_pool)
        .await;
    }

    Ok(Json(ApiResponse::ok(
        serde_json::json!({"id": id.to_string(), "strategy_file": strategy_file}),
    )))
}

pub async fn list_bots(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let bots = sqlx::query_as::<
        _,
        (
            uuid::Uuid,
            String,
            String,
            String,
            String,
            i32,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"SELECT id, name, symbol, exchange, status, leverage,
           created_at, updated_at
           FROM qd_grid_bots WHERE user_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await?;

    let items: Vec<_> = bots
        .iter()
        .map(
            |(
                id,
                name,
                symbol,
                exchange,
                status,
                leverage,
                created_at,
                updated_at,
            )| {
                serde_json::json!({
                    "id": id.to_string(),
                    "name": name,
                    "symbol": symbol,
                    "exchange": exchange,
                    "status": status,
                    "leverage": leverage,
                    "created_at": created_at.to_rfc3339(),
                    "updated_at": updated_at.to_rfc3339(),
                })
            },
        )
        .collect();
    let total = items.len();
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": items,
        "total": total,
    }))))
}

pub async fn get_bot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;


    let bot = sqlx::query_as::<_, virs_models::GridBot>(
        "SELECT * FROM qd_grid_bots WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await?;

    let bot = match bot {
        Some(b) => b,
        None => {
            return Err(VirsError::not_found("Bot not found"));
        }
    };


    let grid_levels: Vec<serde_json::Value> = bot
        .grid_levels_json
        .as_ref()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_else(|| {
            tracing::warn!(bot_id = %id, "grid_levels_json is missing or not an array — returning empty list");
            Vec::new()
        });


    let trades_rows = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            i32,
            String,
            f64,
            f64,
            Option<String>,
            Option<f64>,
            Option<f64>,
            f64,
            chrono::DateTime<chrono::Utc>,
            Option<chrono::DateTime<chrono::Utc>>,
            String,
        ),
    >(
        r#"SELECT
             open_ctx.client_order_id AS open_client_order_id,
             close_ctx.client_order_id AS close_client_order_id,
             open_ctx.grid_level,
             LOWER(open_ord.side) AS open_side,
             open_ord.avg_fill_price::float AS open_price,
             open_ord.filled_qty::float AS open_quantity,
             CASE WHEN close_ord.side IS NOT NULL THEN LOWER(close_ord.side) END AS close_side,
             close_ord.avg_fill_price::float AS close_price,
             close_ord.filled_qty::float AS close_quantity,
             COALESCE(close_ord.realized_pnl::float, 0) AS pnl,
             open_ctx.created_at AS opened_at,
             close_ctx.created_at AS closed_at,
             open_ctx.status
           FROM pe_grid_order_context open_ctx
           JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
           LEFT JOIN pe_grid_order_context close_ctx ON close_ctx.paired_client_order_id = open_ctx.client_order_id
           LEFT JOIN pe_order_latest close_ord ON close_ord.client_order_id = close_ctx.client_order_id
           WHERE open_ctx.bot_id = $1 AND open_ctx.order_role = 'open'
           ORDER BY open_ctx.created_at DESC LIMIT 50"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!(bot_id = %id, error = %e, "Failed to fetch grid trades from database");
        VirsError::config(format!("Failed to fetch trade history: {}", e))
    })?;

    let trades: Vec<serde_json::Value> = trades_rows
        .iter()
        .map(
            |(
                open_cid,
                _close_cid,
                level,
                side,
                open_p,
                open_qty,
                close_side,
                close_p,
                close_qty,
                pnl,
                opened_at,
                closed_at,
                t_status,
            )| {
                let pnl_pct = if *open_p > 0.0 && *open_qty > 0.0 {
                    pnl / (open_p * open_qty) * 100.0
                } else {
                    0.0
                };
                serde_json::json!({
                    "id": open_cid,
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
            },
        )
        .collect();

    // 从全局 PromptLoader 查询策略元数据
    let strategy_detail = if let Some(ref file) = bot.strategy_file {
        let loader = state.prompt_loader.clone();
        loader.get(StrategyType::Grid, file).await.map(|tpl| {
            serde_json::json!({
                "name": tpl.name,
                "description": tpl.description,
                "version": tpl.version,
                "source": tpl.source,
            })
        })
    } else {
        None
    };

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "bot": {
            "id": id.to_string(),
            "name": bot.name,
            "symbol": bot.symbol,
            "exchange": bot.exchange,
            "status": bot.status,
            "is_running": bot.is_running(),
            "is_stopped": bot.is_stopped(),
            "leverage": bot.leverage,
            "grid_count": bot.grid_count,
            "upper_price": bot.upper_price,
            "lower_price": bot.lower_price,
            "grid_spacing": bot.grid_spacing(),
            "grid_profit_pct": bot.grid_profit_pct,
            "quantity_per_grid": bot.quantity_per_grid,
            "initial_capital": bot.initial_capital,
            "total_pnl": bot.total_pnl,
            "total_return_pct": bot.total_return_pct(),
            "unrealized_pnl": bot.unrealized_pnl,
            "total_trades": bot.total_trades,
            "grid_filled_count": bot.grid_filled_count,
            "dynamic_adjust": bot.dynamic_adjust,
            "market_regime": bot.market_regime,
            "ai_analysis": bot.ai_analysis,
            "strategy_file": bot.strategy_file,
            "created_at": bot.created_at.to_rfc3339(),
            "updated_at": bot.updated_at.to_rfc3339(),
        },
        "strategy": strategy_detail,
        "trades": trades,
        "grid_levels": grid_levels,
    }))))
}

/// 更新 bot 配置（当前仅支持变更 strategy_file）。
///
/// 约束：bot 必须处于 stopped 状态才能更新。
/// 前端不暴露此接口，仅作为后端 API 供未来使用。
pub async fn update_bot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    // 查询 bot 当前状态
    let bot = sqlx::query_as::<_, virs_models::GridBot>(
        "SELECT * FROM qd_grid_bots WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await?;

    let bot = match bot {
        Some(b) => b,
        None => return Err(VirsError::not_found("Bot not found")),
    };

    // 运行中拒绝更新
    if bot.is_running() {
        return Err(VirsError::conflict(
            "Cannot update bot while it is running. Please stop the bot first.",
        ));
    }

    // 解析新 strategy_file
    let new_strategy_file = body["strategy_file"]
        .as_str()
        .ok_or_else(|| VirsError::bad_request("strategy_file is required"))?;

    // 校验策略在 loader 中存在
    let loader = state.prompt_loader.clone();
    if loader.get(StrategyType::Grid, new_strategy_file).await.is_none() {
        return Err(VirsError::bad_request(format!(
            "Strategy '{new_strategy_file}' not found in loaded strategies"
        )));
    }

    // UPDATE
    sqlx::query("UPDATE qd_grid_bots SET strategy_file = $2, updated_at = NOW() WHERE id = $1")
        .bind(id)
        .bind(new_strategy_file)
        .execute(&state.db_pool)
        .await?;

    tracing::info!(
        bot_id = %id,
        old_strategy = ?bot.strategy_file,
        new_strategy = %new_strategy_file,
        "Bot strategy updated"
    );

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "id": id.to_string(),
        "strategy_file": new_strategy_file,
    }))))
}

pub async fn start_bot(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, VirsError> {
    let tx = state.engine_manager.grid_cmd_tx().ok_or_else(|| VirsError::Http {
        status: 503,
        message: "Grid engine not running".into(),
    })?;
    tx.send(virs_bot::grid::types::GridCommand::StartBot { bot_id: id })
        .await
        .map_err(|_| VirsError::Http {
            status: 500,
            message: "Failed to send command to grid engine".into(),
        })?;
    Ok(Json(ApiResponse::ok(serde_json::json!({"started": true}))))
}


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
) -> Result<Json<ApiResponse>, VirsError> {
    let tx = state.engine_manager.grid_cmd_tx().ok_or_else(|| VirsError::Http {
        status: 503,
        message: "Grid engine not running".into(),
    })?;
    tx.send(virs_bot::grid::types::GridCommand::StopBot { bot_id: id })
        .await
        .map_err(|_| VirsError::Http {
            status: 500,
            message: "Failed to send command to grid engine".into(),
        })?;
    Ok(Json(ApiResponse::ok(serde_json::json!({"stopped": true}))))
}

pub async fn delete_bot(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, VirsError> {

    if let Some(tx) = state.engine_manager.grid_cmd_tx() {
        tx.send(virs_bot::grid::types::GridCommand::DeleteBot {
            bot_id: id,
            close_position: true,
        })
        .await
        .map_err(|_| VirsError::Http {
            status: 500,
            message: "Failed to send command to grid engine".into(),
        })?;
    } else {

    }

    let result = sqlx::query(r#"DELETE FROM qd_grid_bots WHERE id = $1"#)
        .bind(id)
        .execute(&state.db_pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(VirsError::not_found("Bot not found"));
    }
    Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true}))))
}

pub async fn get_trades(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
    axum::extract::Query(params): axum::extract::Query<TradesQuery>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, 100);
    let offset = (page - 1) * page_size;


    let total: i64 = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM pe_grid_order_context WHERE bot_id = $1 AND user_id = $2 AND order_role = 'open'"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await?;


    let trades = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            i32,
            String,
            f64,
            f64,
            Option<String>,
            Option<f64>,
            Option<f64>,
            f64,
            chrono::DateTime<chrono::Utc>,
            Option<chrono::DateTime<chrono::Utc>>,
            String,
        ),
    >(
        r#"SELECT
             open_ctx.client_order_id AS open_client_order_id,
             close_ctx.client_order_id AS close_client_order_id,
             open_ctx.grid_level,
             LOWER(open_ord.side) AS open_side,
             open_ord.avg_fill_price::float AS open_price,
             open_ord.filled_qty::float AS open_quantity,
             CASE WHEN close_ord.side IS NOT NULL THEN LOWER(close_ord.side) END AS close_side,
             close_ord.avg_fill_price::float AS close_price,
             close_ord.filled_qty::float AS close_quantity,
             COALESCE(close_ord.realized_pnl::float, 0) AS pnl,
             open_ctx.created_at AS opened_at,
             close_ctx.created_at AS closed_at,
             open_ctx.status
           FROM pe_grid_order_context open_ctx
           JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
           LEFT JOIN pe_grid_order_context close_ctx ON close_ctx.paired_client_order_id = open_ctx.client_order_id
           LEFT JOIN pe_order_latest close_ord ON close_ord.client_order_id = close_ctx.client_order_id
           WHERE open_ctx.bot_id = $1 AND open_ctx.user_id = $2 AND open_ctx.order_role = 'open'
           ORDER BY open_ctx.created_at DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.db_pool)
    .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "trades": trades.iter().map(|(open_cid, _close_cid, level, open_side, open_p, open_qty, close_side, close_p, close_qty, pnl, opened_at, closed_at, status)| {
            let pnl_pct = if *open_p > 0.0 && *open_qty > 0.0 {
                pnl / (open_p * open_qty) * 100.0
            } else {
                0.0
            };
            serde_json::json!({
                "id": open_cid,
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
    }))))
}


pub async fn get_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;


    let trades = sqlx::query_as::<_, (f64, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT COALESCE(close_ord.realized_pnl::float, 0) AS pnl, open_ctx.created_at AS opened_at
           FROM pe_grid_order_context open_ctx
           JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
           JOIN pe_grid_order_context close_ctx ON close_ctx.paired_client_order_id = open_ctx.client_order_id AND close_ctx.order_role = 'close'
           JOIN pe_order_latest close_ord ON close_ord.client_order_id = close_ctx.client_order_id
           WHERE open_ctx.bot_id = $1 AND open_ctx.user_id = $2 AND open_ctx.status = 'closed'
           ORDER BY open_ctx.created_at ASC"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await?;


    let bot_stats = sqlx::query_as::<_, (f64, f64, i32, i32)>(
        r#"SELECT total_pnl, unrealized_pnl, total_trades, grid_filled_count FROM qd_grid_bots WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await?;

    let (total_pnl, unrealized_pnl, total_trades, grid_filled_count) = match bot_stats {
        Some((tp, up, tt, gf)) => (tp, up, tt, gf),
        None => (0.0f64, 0.0f64, 0i32, 0i32),
    };


    let realized_pnl: f64 = trades.iter().map(|t| t.0).sum();
    let net_pnl = realized_pnl + unrealized_pnl;


    let win_trades = trades.iter().filter(|t| t.0 > 0.0).count() as i32;
    let loss_trades = trades.iter().filter(|t| t.0 < 0.0).count() as i32;
    let closed_count = trades.len() as i32;
    let win_rate = if closed_count > 0 {
        (win_trades as f64 / closed_count as f64) * 100.0
    } else {
        0.0
    };


    let profits: Vec<f64> = trades.iter().filter(|t| t.0 > 0.0).map(|t| t.0).collect();
    let losses: Vec<f64> = trades.iter().filter(|t| t.0 < 0.0).map(|t| t.0).collect();
    let avg_profit = if !profits.is_empty() {
        profits.iter().sum::<f64>() / profits.len() as f64
    } else {
        0.0
    };
    let avg_loss = if !losses.is_empty() {
        losses.iter().sum::<f64>() / losses.len() as f64
    } else {
        0.0
    };
    let profit_loss_ratio = if avg_loss.abs() > 0.0 {
        avg_profit / avg_loss.abs()
    } else if avg_profit > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };


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


    let avg_hold_time = if trades.len() > 1 {
        let mut total_diff_ms = 0i64;
        let mut count = 0i64;
        for i in 1..trades.len() {
            let diff = trades[i].1.timestamp_millis() - trades[i - 1].1.timestamp_millis();
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


    let avg_pnl = if !trades.is_empty() {
        realized_pnl / trades.len() as f64
    } else {
        0.0
    };


    let max_profit: f64 = trades.iter().map(|t| t.0).fold(0.0f64, |a, b| a.max(b));
    let max_loss: f64 = trades.iter().map(|t| t.0).fold(0.0f64, |a, b| a.min(b));

    Ok(Json(ApiResponse::ok(serde_json::json!({
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
    }))))
}

pub async fn get_analysis_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
    axum::extract::Query(params): axum::extract::Query<TradesQuery>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, 100);
    let offset = (page - 1) * page_size;


    let total: i64 = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM qd_grid_analysis_logs l
           JOIN qd_grid_bots b ON l.bot_id = b.id
           WHERE l.bot_id = $1 AND b.user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await?;

    let logs = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, String, String, String, serde_json::Value, Option<String>, String, String, chrono::DateTime<chrono::Utc>, Option<String>)>(
        r#"SELECT l.id, l.bot_id, l.analysis_type, l.status, l.system_prompt, l.result, l.error, l.user_prompt, l.llm_model, l.created_at, l.strategy_file
           FROM qd_grid_analysis_logs l
           JOIN qd_grid_bots b ON l.bot_id = b.id
           WHERE l.bot_id = $1 AND b.user_id = $2
           ORDER BY l.created_at DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.db_pool)
    .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": logs.iter().map(|(id, bot_id, analysis_type, status, system_prompt, result, error, user_prompt, llm_model, created_at, strategy_file)| {
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
                "strategy_file": strategy_file,
                "created_at": created_at.to_rfc3339(),
            })
        }).collect::<Vec<_>>(),
        "total": total,
        "page": page,
        "page_size": page_size,
    }))))
}
