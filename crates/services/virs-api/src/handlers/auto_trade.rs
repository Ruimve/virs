use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use sqlx::FromRow;
use virs_strategy::prompt::StrategyType;
use virs_error::VirsError;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::handlers::strategy_selection::select_strategy_by_llm;
use crate::handlers::utils::{format_duration, TradesQuery};
use crate::state::AppState;


#[derive(Debug, FromRow)]
struct AutoTradeRow {
    open_client_order_id: String,
    close_client_order_id: Option<String>,
    bot_id: uuid::Uuid,
    symbol: String,
    exchange: String,
    open_side: String,
    open_price: f64,
    open_quantity: f64,
    open_fee: f64,
    opened_at: chrono::DateTime<chrono::Utc>,
    close_side: Option<String>,
    close_price: Option<f64>,
    close_quantity: Option<f64>,
    close_fee: f64,
    closed_at: Option<chrono::DateTime<chrono::Utc>>,
    pnl: f64,
    stop_loss: f64,
    take_profit: f64,
    close_reason: Option<String>,
    status: String,
}

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
    let leverage = body["leverage"].as_i64().ok_or_else(|| {
        VirsError::bad_request("leverage is required and must be greater than 0")
    })? as i32;
    let max_position_pct = body["max_position_pct"].as_f64().ok_or_else(|| {
        VirsError::bad_request("max_position_pct is required and must be between 0 and 100 (exclusive)")
    })?;
    let decide_interval_secs = body["decide_interval_secs"].as_i64().ok_or_else(|| {
        VirsError::bad_request("decide_interval_secs is required and must be greater than 0")
    })? as i32;
    let name = body["name"].as_str().unwrap_or("Auto Bot");
    let paper_mode = body["paper_mode"].as_bool().ok_or_else(|| {
        VirsError::bad_request("paper_mode is required (must be true or false)")
    })?;

    if leverage <= 0 {
        return Err(VirsError::bad_request(
            "leverage is required and must be greater than 0",
        ));
    }
    if max_position_pct <= 0.0 || max_position_pct > 100.0 {
        return Err(VirsError::bad_request(
            "max_position_pct must be between 0 and 100 (exclusive)",
        ));
    }
    if decide_interval_secs <= 0 {
        return Err(VirsError::bad_request(
            "decide_interval_secs must be greater than 0",
        ));
    }


    {
        let auto_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM qd_auto_bots WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&state.db_pool)
                .await?;
        if auto_count > 0 {
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

    // 策略选择：从全局 PromptLoader 获取 auto 策略列表
    let loader = state.prompt_loader.clone();
    let strategies = loader.list(StrategyType::Auto).await;

    let strategy_file = match strategies.len() {
        0 => return Err(VirsError::bad_request(
            "No auto strategy available. Please create a strategy first.",
        )),
        1 => strategies[0].clone(),
        _ => {
            // 多策略：LLM 分析市场数据后选择
            select_strategy_by_llm(&state, &loader, &strategies, exchange, symbol, StrategyType::Auto).await?
        }
    };

    // 校验策略在 loader 中存在
    if loader.get(StrategyType::Auto, &strategy_file).await.is_none() {
        return Err(VirsError::bad_request(
            format!("Strategy '{strategy_file}' not found in loaded strategies"),
        ));
    }

    sqlx::query(
        r#"INSERT INTO qd_auto_bots (id, user_id, name, symbol, exchange, leverage, max_position_pct, decide_interval_secs, paper_mode, initial_capital, status, strategy_file, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'stopped', $11, NOW(), NOW())"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(name)
    .bind(symbol)
    .bind(exchange)
    .bind(leverage)
    .bind(max_position_pct)
    .bind(decide_interval_secs)
    .bind(paper_mode)
    .bind(initial_capital)
    .bind(&strategy_file)
    .execute(&state.db_pool)
    .await?;

    // 如果是 LLM 选择的策略，记录选择日志
    if strategies.len() > 1 {
        let _ = sqlx::query(
            r#"INSERT INTO qd_auto_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status, result, strategy_file, completed_at)
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

    let bots = sqlx::query_as::<_, (
        uuid::Uuid, String, String, String, String, i32, f64, i32,
        chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>,
    )>(
        r#"SELECT id, name, symbol, exchange, status, leverage, max_position_pct, decide_interval_secs,
           created_at, updated_at
           FROM qd_auto_bots WHERE user_id = $1 ORDER BY created_at DESC"#,
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
                max_position_pct,
                decide_interval_secs,
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
                    "max_position_pct": max_position_pct,
                    "decide_interval_secs": decide_interval_secs,
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


    let bot = sqlx::query_as::<_, virs_models::AutoBot>(
        "SELECT * FROM qd_auto_bots WHERE id = $1 AND user_id = $2",
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

    // 从全局 PromptLoader 查询策略元数据（不含完整 prompt 文本）
    let strategy_detail = if let Some(ref file) = bot.strategy_file {
        let loader = state.prompt_loader.clone();
        loader.get(StrategyType::Auto, file).await.map(|tpl| {
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
            "id": bot.id.to_string(),
            "name": bot.name,
            "symbol": bot.symbol,
            "exchange": bot.exchange,
            "status": bot.status,
            "is_running": bot.is_running(),
            "is_stopped": bot.is_stopped(),
            "leverage": bot.leverage,
            "max_position_pct": bot.max_position_pct,
            "decide_interval_secs": bot.decide_interval_secs,
            "initial_capital": bot.initial_capital,
            "market_regime": bot.market_regime,
            "ai_analysis": bot.ai_analysis,
            "total_pnl": bot.total_pnl,
            "total_return_pct": bot.total_return_pct(),
            "total_trades": bot.total_trades,
            "win_trades": bot.win_trades,
            "loss_trades": bot.loss_trades,
            "strategy_file": bot.strategy_file,
            "created_at": bot.created_at.to_rfc3339(),
            "updated_at": bot.updated_at.to_rfc3339(),
        },
        "strategy": strategy_detail,
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
    let bot = sqlx::query_as::<_, virs_models::AutoBot>(
        "SELECT * FROM qd_auto_bots WHERE id = $1 AND user_id = $2",
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
    if loader.get(StrategyType::Auto, new_strategy_file).await.is_none() {
        return Err(VirsError::bad_request(format!(
            "Strategy '{new_strategy_file}' not found in loaded strategies"
        )));
    }

    // UPDATE — include user_id in WHERE as defense-in-depth against TOCTOU
    sqlx::query("UPDATE qd_auto_bots SET strategy_file = $2, updated_at = NOW() WHERE id = $1 AND user_id = $3")
        .bind(id)
        .bind(new_strategy_file)
        .bind(user_id)
        .execute(&state.db_pool)
        .await?;

    tracing::info!(
        bot_id = %id,
        old_strategy = %bot.strategy_file.as_deref().unwrap_or("none"),
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
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    verify_bot_ownership(&state, id, user_id).await?;

    let tx = state.engine_manager.auto_cmd_tx().ok_or_else(|| VirsError::Http {
        status: 503,
        message: "Auto trade engine not running".into(),
    })?;
    tx.send(virs_bot::auto::types::AutoCommand::StartBot { bot_id: id })
        .await
        .map_err(|_| VirsError::Http {
            status: 500,
            message: "Failed to send command to auto trade engine".into(),
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

/// Verify that the bot belongs to the user. Returns 404 (not "forbidden") to avoid
/// leaking existence of other users' bots.
async fn verify_bot_ownership(
    state: &AppState,
    bot_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<(), VirsError> {
    let exists: Option<bool> = sqlx::query_scalar("SELECT true FROM qd_auto_bots WHERE id = $1 AND user_id = $2")
        .bind(bot_id)
        .bind(user_id)
        .fetch_optional(&state.db_pool)
        .await?;
    if exists.is_none() {
        return Err(VirsError::not_found("Bot not found"));
    }
    Ok(())
}

pub async fn stop_bot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    verify_bot_ownership(&state, id, user_id).await?;

    let tx = state.engine_manager.auto_cmd_tx().ok_or_else(|| VirsError::Http {
        status: 503,
        message: "Auto trade engine not running".into(),
    })?;
    tx.send(virs_bot::auto::types::AutoCommand::StopBot { bot_id: id })
        .await
        .map_err(|_| VirsError::Http {
            status: 500,
            message: "Failed to send command to auto trade engine".into(),
        })?;
    Ok(Json(ApiResponse::ok(serde_json::json!({"stopped": true}))))
}

pub async fn delete_bot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    verify_bot_ownership(&state, id, user_id).await?;

    // Engine owns the full delete lifecycle: stop worker → close positions → delete DB row.
    // Handler awaits engine confirmation via oneshot channel before responding.
    let tx = state.engine_manager.auto_cmd_tx().ok_or_else(|| VirsError::Http {
        status: 503,
        message: "Auto trade engine not running — cannot safely delete bot (positions would not be closed)".into(),
    })?;

    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    tx.send(virs_bot::auto::types::AutoCommand::DeleteBot {
        bot_id: id,
        close_position: true,
        response_tx,
    })
    .await
    .map_err(|_| VirsError::Http {
        status: 500,
        message: "Failed to send command to auto trade engine".into(),
    })?;

    match response_rx.await {
        Ok(Ok(())) => Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true})))),
        Ok(Err(msg)) => Err(VirsError::Http {
            status: 500,
            message: format!("Engine failed to delete bot: {msg}"),
        }),
        Err(_) => Err(VirsError::Http {
            status: 500,
            message: "Engine dropped response channel without responding".into(),
        }),
    }
}

pub async fn get_trades(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
    axum::extract::Query(params): axum::extract::Query<TradesQuery>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;

    verify_bot_ownership(&state, id, user_id).await?;

    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, 100);
    let offset = (page - 1) * page_size;


    let total: i64 = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM pe_auto_order_context WHERE bot_id = $1 AND user_id = $2 AND order_role = 'open'"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await?;


    let trades = sqlx::query_as::<_, AutoTradeRow>(
        r#"SELECT
             open_ctx.client_order_id AS open_client_order_id,
             close_ctx.client_order_id AS close_client_order_id,
             open_ctx.bot_id,
             open_ctx.symbol,
             open_ctx.exchange,
             LOWER(open_ord.side) AS open_side,
             open_ord.avg_fill_price::float AS open_price,
             open_ord.filled_qty::float AS open_quantity,
             open_ord.commission::float AS open_fee,
             open_ctx.created_at AS opened_at,
             CASE WHEN close_ord.side IS NOT NULL THEN LOWER(close_ord.side) END AS close_side,
             close_ord.avg_fill_price::float AS close_price,
             close_ord.filled_qty::float AS close_quantity,
             COALESCE(close_ord.commission::float, 0) AS close_fee,
             close_ctx.created_at AS closed_at,
             COALESCE(close_ord.realized_pnl::float, 0) AS pnl,
             open_ctx.stop_loss,
             open_ctx.take_profit,
             close_ctx.close_reason,
             open_ctx.status
           FROM pe_auto_order_context open_ctx
           JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
           LEFT JOIN pe_auto_order_context close_ctx ON close_ctx.paired_client_order_id = open_ctx.client_order_id
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
        "trades": trades.iter().map(|t| {
            let net_pnl = t.pnl - t.open_fee - t.close_fee;
            let pnl_pct = if t.open_price > 0.0 && t.open_quantity > 0.0 {
                net_pnl / (t.open_price * t.open_quantity) * 100.0
            } else {
                0.0
            };
            serde_json::json!({
                "id": t.open_client_order_id,
                "bot_id": t.bot_id.to_string(),
                "symbol": t.symbol,
                "exchange": t.exchange,
                "open_side": t.open_side,
                "open_price": t.open_price,
                "open_quantity": t.open_quantity,
                "open_order_id": t.open_client_order_id,
                "open_fee": t.open_fee,
                "opened_at": t.opened_at.to_rfc3339(),
                "close_side": t.close_side,
                "close_price": t.close_price,
                "close_quantity": t.close_quantity,
                "close_order_id": t.close_client_order_id,
                "close_fee": t.close_fee,
                "closed_at": t.closed_at.map(|c| c.to_rfc3339()),
                "pnl": t.pnl,
                "net_pnl": net_pnl,
                "pnl_pct": pnl_pct,
                "stop_loss": t.stop_loss,
                "take_profit": t.take_profit,
                "close_reason": t.close_reason,
                "status": t.status,
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

    verify_bot_ownership(&state, id, user_id).await?;

    let trades = sqlx::query_as::<
        _,
        (
            f64,
            f64,
            f64,
            f64,
            f64,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"SELECT
             open_ord.avg_fill_price::float AS open_price,
             open_ord.filled_qty::float AS open_quantity,
             open_ord.commission::float AS open_fee,
             COALESCE(close_ord.commission::float, 0) AS close_fee,
             COALESCE(close_ord.realized_pnl::float, 0) AS pnl,
             open_ctx.created_at AS opened_at,
             close_ctx.created_at AS closed_at
           FROM pe_auto_order_context open_ctx
           JOIN pe_order_latest open_ord ON open_ord.client_order_id = open_ctx.client_order_id
           JOIN pe_auto_order_context close_ctx ON close_ctx.paired_client_order_id = open_ctx.client_order_id AND close_ctx.order_role = 'close'
           JOIN pe_order_latest close_ord ON close_ord.client_order_id = close_ctx.client_order_id
           WHERE open_ctx.bot_id = $1 AND open_ctx.user_id = $2 AND open_ctx.status = 'closed'
           ORDER BY close_ctx.created_at ASC"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await?;


    let bot = sqlx::query_as::<_, virs_models::AutoBot>(
        "SELECT * FROM qd_auto_bots WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await?;

    let bot = match bot {
        Some(b) => b,
        None => return Err(VirsError::not_found("Bot not found")),
    };

    let total_trades = bot.total_trades;
    let win_trades = bot.win_trades;
    let loss_trades = bot.loss_trades;
    let win_rate = bot.win_rate();
    let loss_rate = bot.loss_rate();

    // Compute net PnL per trade (gross realized_pnl - open_fee - close_fee) for all analytics.
    // This aligns with the worker's net PnL calculation (events.rs: realized_pnl = gross_pnl - total_fee).
    let net_pnl_per_trade: Vec<f64> = trades.iter().map(|t| t.4 - t.2 - t.3).collect();

    // Win/loss boundary aligned with worker (events.rs L607: realized_pnl >= 0.0 → win).
    let profits: Vec<f64> = net_pnl_per_trade.iter().filter(|&&p| p >= 0.0).copied().collect();
    let losses: Vec<f64> = net_pnl_per_trade.iter().filter(|&&p| p < 0.0).copied().collect();
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
    for &pnl in &net_pnl_per_trade {
        cumulative += pnl;
        if cumulative > peak {
            peak = cumulative;
        }
        let drawdown = peak - cumulative;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }


    let mut total_hold_ms = 0i64;
    let mut pair_count = 0i64;
    for t in &trades {
        let hold_ms = t.6.timestamp_millis() - t.5.timestamp_millis();
        if hold_ms > 0 {
            total_hold_ms += hold_ms;
            pair_count += 1;
        }
    }
    let avg_hold_ms = if pair_count > 0 {
        total_hold_ms / pair_count
    } else {
        0
    };
    let avg_hold_time = format_duration(avg_hold_ms);


    let mut max_win_streak = 0i32;
    let mut max_loss_streak = 0i32;
    let mut current_win = 0i32;
    let mut current_loss = 0i32;
    for &pnl in &net_pnl_per_trade {
        if pnl >= 0.0 {
            current_win += 1;
            current_loss = 0;
            if current_win > max_win_streak {
                max_win_streak = current_win;
            }
        } else if pnl < 0.0 {
            current_loss += 1;
            current_win = 0;
            if current_loss > max_loss_streak {
                max_loss_streak = current_loss;
            }
        }
    }


    let total_fee: f64 = trades.iter().map(|t| t.2 + t.3).sum();
    let gross_pnl: f64 = trades.iter().map(|t| t.4).sum();


    let total_volume: f64 = trades.iter().map(|t| t.0 * t.1).sum();

    let net_pnl_after_fee = gross_pnl - total_fee;

    let avg_pnl = if !trades.is_empty() {
        net_pnl_after_fee / trades.len() as f64
    } else {
        0.0
    };


    let max_profit: f64 = net_pnl_per_trade.iter().fold(0.0f64, |a, &b| a.max(b));
    let max_loss: f64 = net_pnl_per_trade.iter().fold(0.0f64, |a, &b| a.min(b));

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "win_rate": win_rate,
        "loss_rate": loss_rate,
        "profit_loss_ratio": profit_loss_ratio,
        "max_drawdown": max_drawdown,
        "avg_hold_time": avg_hold_time,
        "max_win_streak": max_win_streak,
        "max_loss_streak": max_loss_streak,
        "total_fee": total_fee,
        "gross_pnl": gross_pnl,
        "net_pnl_after_fee": net_pnl_after_fee,
        "total_pnl": bot.total_pnl,
        "total_trades": total_trades,
        "win_trades": win_trades,
        "loss_trades": loss_trades,
        "total_volume": total_volume,
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

    verify_bot_ownership(&state, id, user_id).await?;

    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, 100);
    let offset = (page - 1) * page_size;


    let total: i64 = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM qd_auto_analysis_logs l
           JOIN qd_auto_bots b ON l.bot_id = b.id
           WHERE l.bot_id = $1 AND b.user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await?;

    let logs = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, String, String, String, serde_json::Value, Option<String>, String, String, chrono::DateTime<chrono::Utc>, Option<String>)>(
        r#"SELECT l.id, l.bot_id, l.analysis_type, l.status, l.system_prompt, l.result, l.error, l.user_prompt, l.llm_model, l.created_at, l.strategy_file
           FROM qd_auto_analysis_logs l
           JOIN qd_auto_bots b ON l.bot_id = b.id
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
