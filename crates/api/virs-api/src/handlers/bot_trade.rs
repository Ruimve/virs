use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::Serialize;
use virs_database as db;
use virs_prompt::PromptTemplate;
use virs_type::StrategyType;
use virs_error::VirsError;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::handlers::strategy_selection::select_strategy_by_llm;
use crate::handlers::utils::{format_duration, TradesQuery};
use crate::state::AppState;


#[derive(Serialize)]
struct BotInfo {
    id: String,
    name: String,
    symbol: String,
    exchange: String,
    status: String,
    bot_type: String,
    is_running: bool,
    is_stopped: bool,
    leverage: i32,
    max_position_pct: f64,
    decide_interval_secs: i32,
    initial_capital: f64,
    market_regime: Option<String>,
    ai_analysis: Option<String>,
    total_pnl: f64,
    total_return_pct: f64,
    total_trades: i32,
    win_trades: i32,
    loss_trades: i32,
    strategy_file: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<&virs_type::Bot> for BotInfo {
    fn from(bot: &virs_type::Bot) -> Self {
        Self {
            id: bot.id.to_string(),
            name: bot.name.clone(),
            symbol: bot.symbol.clone(),
            exchange: bot.exchange.clone(),
            status: bot.status.clone(),
            bot_type: bot.bot_type.clone(),
            is_running: bot.is_running(),
            is_stopped: bot.is_stopped(),
            leverage: bot.leverage,
            max_position_pct: bot.max_position_pct,
            decide_interval_secs: bot.decide_interval_secs,
            initial_capital: bot.initial_capital,
            market_regime: bot.market_regime.clone(),
            ai_analysis: bot.ai_analysis.clone(),
            total_pnl: bot.total_pnl,
            total_return_pct: bot.total_return_pct(),
            total_trades: bot.total_trades,
            win_trades: bot.win_trades,
            loss_trades: bot.loss_trades,
            strategy_file: bot.strategy_file.clone(),
            created_at: bot.created_at.to_rfc3339(),
            updated_at: bot.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
struct StrategyInfo {
    name: String,
    description: String,
    version: i32,
}

impl From<PromptTemplate> for StrategyInfo {
    fn from(tpl: PromptTemplate) -> Self {
        Self {
            name: tpl.name,
            description: tpl.description,
            version: tpl.version,
        }
    }
}

#[derive(Serialize)]
struct BotDetailResponse {
    bot: BotInfo,
    strategy: Option<StrategyInfo>,
}

async fn fetch_strategy(
    loader: &virs_prompt::PromptLoader,
    bot: &virs_type::Bot,
) -> Option<StrategyInfo> {
    let file = bot.strategy_file.as_ref()?;
    loader
        .get(StrategyType::Chat, file)
        .await
        .map(StrategyInfo::from)
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
    let name = body["name"].as_str().unwrap_or("Bot");
    let paper_mode = body["paper_mode"].as_bool().ok_or_else(|| {
        VirsError::bad_request("paper_mode is required (must be true or false)")
    })?;
    let auto_optimize = body["auto_optimize"].as_bool().ok_or_else(|| {
        VirsError::bad_request("auto_optimize is required (must be true or false)")
    })?;
    let bot_type = body["bot_type"]
        .as_str()
        .filter(|s| *s == "chat" || *s == "agent")
        .unwrap_or("chat");

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


    /* 每个用户只能创建一个bot：避免多bot同时操作同一账户导致资金冲突 */
    {
        let bot_count = db::count_bots_by_user(&state.db_pool, user_id).await?;
        if bot_count > 0 {
            return Err(VirsError::conflict(
                "Each account can only have one bot. Please delete your existing bot first.",
            ));
        }
    }


    state.engine_manager.ensure_started(paper_mode).await?;


    let exchange_key = format!("{}:{}", exchange, virs_type::MarketType::Perpetual);
    if state.exchange_registry.get(&exchange_key).is_none() {
        return Err(VirsError::Http {
            status: 412,
            message: "Exchange not registered. Please save API credentials first.".into(),
        });
    }


    state.kline_engine.subscribe_market(exchange, symbol, virs_type::MarketType::Perpetual).await?;


    let fallback = if paper_mode { 10000.0 } else { 0.0 };
    let initial_capital = match state.exchange_registry.get(&exchange_key) {
        Some(ex) => {

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


    let loader = state.prompt_loader.clone();
    let strategies = loader.list(StrategyType::Chat).await;

    /* strategy_file必须在创建时绑定：无策略时拒绝创建，多策略时由LLM自动选择 */
    let strategy_file = match strategies.len() {
        0 => return Err(VirsError::bad_request(
            "No strategy available. Please create a strategy first.",
        )),
        1 => strategies[0].clone(),
        _ => {

            select_strategy_by_llm(&state, &loader, &strategies, exchange, symbol, StrategyType::Chat).await?
        }
    };


    if loader.get(StrategyType::Chat, &strategy_file).await.is_none() {
        return Err(VirsError::bad_request(
            format!("Strategy '{strategy_file}' not found in loaded strategies"),
        ));
    }

    db::insert_bot(&state.db_pool, id, user_id, name, symbol, exchange, leverage, max_position_pct, decide_interval_secs, paper_mode, initial_capital, bot_type, &strategy_file, auto_optimize).await?;


    if strategies.len() > 1 {
        let _ = db::insert_strategy_selection_log(&state.db_pool, id, "You are a trading strategy selector. Choose the best strategy for the current market conditions.", &format!("Symbol: {}, Exchange: {}, Strategies: {:?}", symbol, exchange, strategies), &serde_json::json!({"selected": strategy_file}), &strategy_file).await;
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

    let bots = db::list_bots_by_user(&state.db_pool, user_id).await?;

    let loader = &state.prompt_loader;
    let mut items = Vec::with_capacity(bots.len());
    for bot in &bots {
        let strategy = fetch_strategy(loader, bot).await;
        items.push(BotDetailResponse {
            bot: BotInfo::from(bot),
            strategy,
        });
    }
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

    let bot = db::get_bot_by_id(&state.db_pool, id, user_id).await?;

    let bot = match bot {
        Some(b) => b,
        None => return Err(VirsError::not_found("Bot not found")),
    };

    let strategy = fetch_strategy(&state.prompt_loader, &bot).await;

    Ok(Json(ApiResponse::ok(serde_json::json!(BotDetailResponse {
        bot: BotInfo::from(&bot),
        strategy,
    }))))
}


pub async fn update_bot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let user_id = extract_user_id(&headers, &state.jwt_secret)?;


    let bot = db::get_bot_by_id(&state.db_pool, id, user_id).await?;

    let bot = match bot {
        Some(b) => b,
        None => return Err(VirsError::not_found("Bot not found")),
    };


    /* 运行中的bot不能更新strategy_file：必须先停止bot再更新策略，然后重新启动 */
    if bot.is_running() {
        return Err(VirsError::conflict(
            "Cannot update bot while it is running. Please stop the bot first.",
        ));
    }


    let new_strategy_file = body["strategy_file"]
        .as_str()
        .ok_or_else(|| VirsError::bad_request("strategy_file is required"))?;


    let loader = state.prompt_loader.clone();
    if loader.get(StrategyType::Chat, new_strategy_file).await.is_none() {
        return Err(VirsError::bad_request(format!(
            "Strategy '{new_strategy_file}' not found in loaded strategies"
        )));
    }


    db::update_bot_strategy(&state.db_pool, id, new_strategy_file, user_id).await?;

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

    let tx = state.engine_manager.bot_cmd_tx().ok_or_else(|| VirsError::Http {
        status: 503,
        message: "Trade engine not running".into(),
    })?;
    tx.send(virs_type::BotCommand::StartBot { bot_id: id })
        .await
        .map_err(|_| VirsError::Http {
            status: 503,
            message: "Failed to send command to trade engine".into(),
        })?;
    Ok(Json(ApiResponse::ok(serde_json::json!({"started": true}))))
}


fn extract_quote_asset(symbol: &str) -> String {
    let upper = symbol.to_uppercase();
    for quote in &["USDT", "USDC", "FDUSD", "BUSD", "TUSD", "BTC", "ETH", "BNB"] {
        if upper.ends_with(quote) {
            return quote.to_string();
        }
    }
    "USDT".to_string()
}


async fn verify_bot_ownership(
    state: &AppState,
    bot_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<(), VirsError> {
    let exists = db::verify_bot_ownership(&state.db_pool, bot_id, user_id).await?;
    if !exists {
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

    let tx = state.engine_manager.bot_cmd_tx().ok_or_else(|| VirsError::Http {
        status: 503,
        message: "Trade engine not running".into(),
    })?;
    tx.send(virs_type::BotCommand::StopBot { bot_id: id })
        .await
        .map_err(|_| VirsError::Http {
            status: 503,
            message: "Failed to send command to trade engine".into(),
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


    let tx = state.engine_manager.bot_cmd_tx().ok_or_else(|| VirsError::Http {
        status: 503,
        message: "Trade engine not running — cannot safely delete bot (positions would not be closed)".into(),
    })?;

    /* 删除bot前先平仓：通过oneshot channel同步等待引擎确认，确保持仓已清理 */
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    tx.send(virs_type::BotCommand::DeleteBot {
        bot_id: id,
        close_position: true,
        response_tx,
    })
    .await
    .map_err(|_| VirsError::Http {
        status: 503,
        message: "Failed to send command to trade engine".into(),
    })?;

    match response_rx.await {
        Ok(Ok(())) => Ok(Json(ApiResponse::ok(serde_json::json!({"deleted": true})))),
        Ok(Err(msg)) => Err(VirsError::Http {
            status: 503,
            message: format!("Engine failed to delete bot: {msg}"),
        }),
        Err(_) => Err(VirsError::Http {
            status: 503,
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


    let total = db::count_bot_trades(&state.db_pool, id, user_id).await?;


    let trades = db::query_bot_trades(&state.db_pool, id, user_id, page_size as i64, offset as i64).await?;

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

    let trades = db::get_bot_trade_stats(&state.db_pool, id, user_id).await?;


    let bot = db::get_bot_by_id(&state.db_pool, id, user_id).await?;

    let bot = match bot {
        Some(b) => b,
        None => return Err(VirsError::not_found("Bot not found")),
    };

    let total_trades = bot.total_trades;
    let win_trades = bot.win_trades;
    let loss_trades = bot.loss_trades;
    let win_rate = bot.win_rate();
    let loss_rate = bot.loss_rate();


    let net_pnl_per_trade: Vec<f64> = trades.iter().map(|t| t.4 - t.2 - t.3).collect();


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


    /* 计算最大回撤：按时间顺序累加净盈亏，追踪峰值并计算峰值到谷值的最大落差 */
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


    let total = db::count_analysis_logs(&state.db_pool, id, user_id).await?;

    let logs = db::query_analysis_logs(&state.db_pool, id, user_id, page_size as i64, offset as i64).await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": logs.iter().map(|(id, bot_id, analysis_type, status, system_prompt, result, error, user_prompt, llm_model, created_at, strategy_file, execution_status, intercept_reason, completed_at)| {
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
                "execution_status": execution_status,
                "intercept_reason": intercept_reason,
                "completed_at": completed_at.map(|t| t.to_rfc3339()),
            })
        }).collect::<Vec<_>>(),
        "total": total,
        "page": page,
        "page_size": page_size,
    }))))
}
