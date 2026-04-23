use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::AppState;
use crate::api::middleware::AuthUser;
use crate::engine::backtest::BacktestEngine;
use crate::exchange::Exchange;
use crate::models::*;

#[derive(Deserialize)]
pub struct BacktestListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct BacktestDetailRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub strategy_name: String,
    pub symbol: String,
    pub exchange: String,
    pub timeframe: String,
    pub start_date: chrono::DateTime<chrono::Utc>,
    pub end_date: chrono::DateTime<chrono::Utc>,
    pub initial_balance: f64,
    pub final_balance: f64,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub win_rate: f64,
    pub total_trades: i64,
    pub profit_trades: i64,
    pub loss_trades: i64,
    pub avg_profit: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
    pub max_consecutive_wins: i64,
    pub max_consecutive_losses: i64,
    pub trades_json: serde_json::Value,
    pub equity_curve_json: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct BacktestListRow {
    pub id: Uuid,
    pub strategy_name: String,
    pub symbol: String,
    pub exchange: String,
    pub timeframe: String,
    pub start_date: chrono::DateTime<chrono::Utc>,
    pub end_date: chrono::DateTime<chrono::Utc>,
    pub initial_balance: f64,
    pub final_balance: f64,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub sharpe_ratio: f64,
    pub win_rate: f64,
    pub total_trades: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn run_backtest(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(req): Json<BacktestRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let market_type = req.trading_config
        .get("market_type")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "perpetual" => MarketType::Perpetual,
            _ => MarketType::Spot,
        })
        .unwrap_or(MarketType::Spot);

    let exchange_key = super::market::ensure_exchange(&state, &req.exchange, market_type).await?;
    let exchange = state.strategy_engine.get_exchange(&exchange_key).unwrap();

    let parse_date = |s: &Option<String>| -> Option<chrono::DateTime<chrono::Utc>> {
        s.as_ref().and_then(|d| {
            if d.is_empty() {
                return None;
            }
            chrono::DateTime::parse_from_rfc3339(&format!("{}T00:00:00Z", d.trim()))
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok()
                .or_else(|| chrono::DateTime::parse_from_rfc3339(d).map(|dt| dt.with_timezone(&chrono::Utc)).ok())
        })
    };

    let start_dt = parse_date(&req.start_date);
    let end_dt = parse_date(&req.end_date);

    let duration_secs = match (start_dt, end_dt) {
        (Some(s), Some(e)) => (e.timestamp() - s.timestamp()).max(0),
        _ => 30 * 24 * 3600,
    };
    let interval_secs = match req.timeframe.as_str() {
        "1m" => 60, "5m" => 300, "15m" => 900,
        "1h" => 3600, "4h" => 14400, "1d" => 86400,
        _ => 3600,
    };
    let estimated_candles = (duration_secs / interval_secs) as u32;
    let max_candles = match req.timeframe.as_str() {
        "1m" | "5m" | "15m" => 500,
        "1h" | "4h" => 500,
        "1d" | "1w" => 365,
        _ => 500,
    };
    let limit = estimated_candles.min(max_candles).max(100);

    let since_ms = start_dt.map(|dt| dt.timestamp_millis());

    let klines = match exchange.get_klines(&req.symbol, &req.timeframe, limit, since_ms).await {
        Ok(k) if !k.is_empty() => k,
        Ok(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "No kline data available for {} ({}) on {}. Cannot run backtest without real data. Please verify the symbol and timeframe are correct.",
                    req.symbol, req.timeframe, req.exchange
                ))),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "Failed to fetch kline data for backtest: {}", e
                ))),
            ));
        }
    };

    let commission = req.trading_config
        .get("commission_rate")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.001);

    let slippage = req.trading_config
        .get("slippage")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0005);

    let stop_loss = req.trading_config
        .get("stop_loss_pct")
        .and_then(|v| v.as_f64());

    let take_profit = req.trading_config
        .get("take_profit_pct")
        .and_then(|v| v.as_f64());

    let position_pct = req.trading_config
        .get("position_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);

    let trailing_stop_pct = req.trading_config
        .get("trailing_stop_pct")
        .and_then(|v| v.as_f64());

    let trailing_activation_pct = req.trading_config
        .get("trailing_activation_pct")
        .and_then(|v| v.as_f64());

    let trade_direction = req.trading_config
        .get("trade_direction")
        .and_then(|v| v.as_str())
        .unwrap_or("long");

    let leverage = req.trading_config
        .get("leverage")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u32;

    let engine = BacktestEngine::new(req.initial_balance, commission, slippage);

    // Determine signal generation method: script-based or plugin-based
    let is_script = req.indicator_config
        .get("strategy_code")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    let result = if is_script {
        use crate::engine::lua_executor::{LuaExecutor, LuaExecutorConfig};
        let executor = LuaExecutor::new(LuaExecutorConfig::default());
        let code = req.indicator_config.get("strategy_code").and_then(|v| v.as_str()).unwrap_or("");

        let mut script_params: HashMap<String, f64> = HashMap::new();
        if let Some(obj) = req.indicator_config.as_object() {
            for (key, value) in obj {
                if key == "plugin" || key == "strategy_code" { continue; }
                if let Some(num) = value.as_f64() {
                    script_params.insert(key.clone(), num);
                }
            }
        }

        let mut signals: Vec<i8> = Vec::with_capacity(klines.len());
        if let Err(e) = executor.execute_backtest(code, &klines, &script_params, |signal| {
            signals.push(signal);
        }) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(&format!("Lua execution error: {}", e))),
            ));
        }

        engine.run(&klines, |_, idx| {
            signals.get(idx).copied().unwrap_or(0)
        }, stop_loss, take_profit, position_pct, trailing_stop_pct, trailing_activation_pct, trade_direction, start_dt, end_dt, leverage)
    } else {
        // Plugin-based mode (existing logic)
        let plugin_name = req
            .indicator_config
            .get("plugin")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                let legacy = req.strategy_type.as_str();
                match legacy {
                    "sma_crossover" | "rsi" | "macd" | "bollinger_bands" => legacy.to_string(),
                    _ => "sma_crossover".to_string(),
                }
            });

        let mut params: HashMap<String, f64> = HashMap::new();
        if let Some(obj) = req.indicator_config.as_object() {
            for (key, value) in obj {
                if key == "plugin" {
                    continue;
                }
                if let Some(num) = value.as_f64() {
                    params.insert(key.clone(), num);
                }
            }
        }

        if !req.indicator_config.get("plugin").is_some() {
            match plugin_name.as_str() {
                "sma_crossover" => {
                    if let Some(v) = params.remove("short_period") {
                        params.insert("fast_period".into(), v);
                    }
                    if let Some(v) = params.remove("long_period") {
                        params.insert("slow_period".into(), v);
                    }
                }
                "macd" => {
                    if let Some(v) = params.remove("fast_period") {
                        params.insert("fast_period".into(), v);
                    }
                    if let Some(v) = params.remove("slow_period") {
                        params.insert("slow_period".into(), v);
                    }
                    if let Some(v) = params.remove("signal_period") {
                        params.insert("signal_period".into(), v);
                    }
                }
                _ => {}
            }
        }

        let plugin_registry = &state.plugin_registry;
        engine.run(&klines, |klines, idx| {
            plugin_registry
                .generate_signal(&plugin_name, klines, idx, &params)
                .unwrap_or(0)
        }, stop_loss, take_profit, position_pct, trailing_stop_pct, trailing_activation_pct, trade_direction, start_dt, end_dt, leverage)
    };

    let _ = sqlx::query(
        r#"INSERT INTO qd_backtest_results
           (id, user_id, strategy_name, symbol, exchange, timeframe, start_date, end_date,
            initial_balance, final_balance, total_return_pct, max_drawdown_pct,
            sharpe_ratio, sortino_ratio, win_rate, total_trades, profit_trades, loss_trades,
            avg_profit, avg_loss, profit_factor, max_consecutive_wins, max_consecutive_losses,
            trades_json, equity_curve_json, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, NOW())"#,
    )
    .bind(result.id)
    .bind(result.user_id)
    .bind(&result.strategy_name)
    .bind(&result.symbol)
    .bind(&result.exchange)
    .bind(&result.timeframe)
    .bind(result.start_date)
    .bind(result.end_date)
    .bind(result.initial_balance)
    .bind(result.final_balance)
    .bind(result.total_return_pct)
    .bind(result.max_drawdown_pct)
    .bind(result.sharpe_ratio)
    .bind(result.sortino_ratio)
    .bind(result.win_rate)
    .bind(result.total_trades)
    .bind(result.profit_trades)
    .bind(result.loss_trades)
    .bind(result.avg_profit)
    .bind(result.avg_loss)
    .bind(result.profit_factor)
    .bind(result.max_consecutive_wins)
    .bind(result.max_consecutive_losses)
    .bind(serde_json::to_value(&result.trades).unwrap_or_default())
    .bind(serde_json::to_value(&result.equity_curve).unwrap_or_default())
    .execute(&state.db_pool)
    .await;

    // Attach klines data for chart rendering
    let klines_data: Vec<serde_json::Value> = klines.iter().map(|k| {
        serde_json::json!({
            "time": k.open_time / 1000,
            "open": k.open,
            "high": k.high,
            "low": k.low,
            "close": k.close,
            "volume": k.volume,
        })
    }).collect();

    let mut result_json = serde_json::to_value(&result).unwrap_or_default();
    if let Some(obj) = result_json.as_object_mut() {
        obj.insert("klines".to_string(), serde_json::json!(klines_data));
    }

    Ok(Json(ApiResponse::ok(result_json)))
}

pub async fn get_backtest_result(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let row = sqlx::query_as::<_, BacktestDetailRow>(
        r#"SELECT id, user_id, strategy_name, symbol, exchange, timeframe,
           start_date, end_date, initial_balance, final_balance,
           total_return_pct, max_drawdown_pct, sharpe_ratio, sortino_ratio,
           win_rate, total_trades, profit_trades, loss_trades,
           avg_profit, avg_loss, profit_factor,
           max_consecutive_wins, max_consecutive_losses,
           trades_json, equity_curve_json, created_at
           FROM qd_backtest_results WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!("Database error: {}", e))),
        )
    })?;

    match row {
        Some(r) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "id": r.id,
            "strategy_name": r.strategy_name,
            "symbol": r.symbol,
            "exchange": r.exchange,
            "timeframe": r.timeframe,
            "start_date": r.start_date,
            "end_date": r.end_date,
            "initial_balance": r.initial_balance,
            "final_balance": r.final_balance,
            "total_return_pct": r.total_return_pct,
            "max_drawdown_pct": r.max_drawdown_pct,
            "sharpe_ratio": r.sharpe_ratio,
            "sortino_ratio": r.sortino_ratio,
            "win_rate": r.win_rate,
            "total_trades": r.total_trades,
            "profit_trades": r.profit_trades,
            "loss_trades": r.loss_trades,
            "avg_profit": r.avg_profit,
            "avg_loss": r.avg_loss,
            "profit_factor": r.profit_factor,
            "max_consecutive_wins": r.max_consecutive_wins,
            "max_consecutive_losses": r.max_consecutive_losses,
            "trades": r.trades_json,
            "equity_curve": r.equity_curve_json,
        })))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("Backtest result not found")),
        )),
    }
}

pub async fn list_backtest_results(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Query(params): Query<BacktestListQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let (page, page_size) = (params.page.unwrap_or(1), params.page_size.unwrap_or(20));
    let offset = (page - 1) * page_size;

    let rows = sqlx::query_as::<_, BacktestListRow>(
        r#"SELECT id, strategy_name, symbol, exchange, timeframe,
           start_date, end_date, initial_balance, final_balance,
           total_return_pct, max_drawdown_pct, sharpe_ratio, win_rate,
           total_trades, created_at
           FROM qd_backtest_results
           ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
    )
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

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM qd_backtest_results")
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": rows,
        "total": total,
        "page": page,
        "page_size": page_size,
    }))))
}
