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
use crate::engine::indicators;
use crate::models::*;
use crate::services::ai::{AiService, AiUserConfig};
use crate::utils::crypto;

// ── Request / Response Types ──

#[derive(Debug, Deserialize)]
pub struct AnalyzeRequest {
    pub symbol: String,
    pub exchange: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBotRequest {
    pub name: String,
    pub symbol: String,
    pub exchange: Option<String>,
    pub upper_price: Option<f64>,
    pub lower_price: Option<f64>,
    pub grid_count: Option<i32>,
    pub grid_profit_pct: Option<f64>,
    pub quantity_per_grid: Option<f64>,
    pub leverage: Option<i32>,
    pub dynamic_adjust: Option<bool>,
    pub adjust_interval_secs: Option<i32>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
    pub market_regime: Option<String>,
    pub ai_analysis: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReanalyzeRequest {
    pub provider: Option<String>,
    pub model: Option<String>,
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

async fn load_user_ai_config(
    db_pool: &sqlx::PgPool,
    user_id: &Uuid,
    encryption_key: &[u8; 32],
) -> AiUserConfig {
    #[derive(Debug, sqlx::FromRow)]
    struct EncryptedRow {
        pub provider: String,
        pub encrypted_api_key: String,
    }

    let rows = sqlx::query_as::<_, EncryptedRow>(
        r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials WHERE user_id = $1"#,
    )
    .bind(user_id)
    .fetch_all(db_pool)
    .await;

    let mut config = AiUserConfig::default();

    if let Ok(rows) = rows {
        for row in rows {
            let decrypted = match crypto::decrypt(&row.encrypted_api_key, encryption_key) {
                Ok(key) => key,
                Err(e) => {
                    tracing::warn!(
                        "Failed to decrypt AI credential for provider {}: {}",
                        row.provider,
                        e
                    );
                    continue;
                }
            };

            match row.provider.as_str() {
                "openrouter" => config.openrouter_api_key = Some(decrypted),
                "openai" => config.openai_api_key = Some(decrypted),
                "deepseek" => config.deepseek_api_key = Some(decrypted),
                _ => {
                    tracing::warn!("Unknown AI provider in database: {}", row.provider);
                }
            }
        }
    } else {
        tracing::warn!("Failed to query user AI credentials for user {}", user_id);
    }

    config
}

fn default_grid_system_prompt() -> &'static str {
    r#"你是一位专业的加密货币量化交易分析师，专注于合约网格交易策略。根据提供的市场数据分析当前市场状态，并生成最优的网格交易参数。

## 分析要求
1. 判断市场状态（trending_up/trending_down/ranging/volatile）
2. 确定合理的网格上下界（基于支撑/阻力位、ATR、BBands）
3. 计算最优网格数量和每格利润率
4. 评估风险并给出建议

## 输出格式（严格 JSON，不要 markdown 代码块）
{
  "market_regime": "ranging|trending_up|trending_down|volatile",
  "upper_price": 数字（网格上界）,
  "lower_price": 数字（网格下界）,
  "grid_count": 数字（网格数量，建议 10-50）,
  "grid_profit_pct": 数字（每格利润率%，建议 0.3-2.0）,
  "quantity_per_grid": 数字（每格数量，USDT计）,
  "leverage": 数字（杠杆倍数，建议 1-10）,
  "analysis": "详细分析说明（200字以内）",
  "risk_warning": "风险提示（100字以内）"
}"#
}

const DEFAULT_USER_PROMPT_TEMPLATE: &str = r#"请分析以下市场数据并生成网格交易参数：

## 交易对
{symbol} ({exchange}) - 永续合约

## 近期K线数据（最近30根1h）
{ohlcv_table}

## 关键指标
- RSI(14): {rsi}
- ATR(14): {atr}
- BBands Width: {bb_width}
- EMA(12): {ema12} (方向: {ema12_trend})
- EMA(26): {ema26} (方向: {ema26_trend})
- 价格区间: {price_low} - {price_high}
- 当前价格: {current_price}
- 4h EMA(26): {ema_4h}
- 24h 波动率: {volatility}%

请生成适合当前市场状态的网格交易参数。"#;

fn strip_code_fences(content: &str) -> String {
    let content = content.trim();
    if content.starts_with("```json") {
        content
            .trim_start_matches("```json")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else if content.starts_with("```") {
        content
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else {
        content.to_string()
    }
}

// ── Shared indicator computation ──

struct GridIndicators {
    current_price: f64,
    rsi: f64,
    atr: f64,
    bb_width: f64,
    ema12: f64,
    ema26: f64,
    ema12_trend: &'static str,
    ema26_trend: &'static str,
    price_high: f64,
    price_low: f64,
    ema_4h: f64,
    volatility: f64,
    ohlcv_table: String,
}

fn compute_grid_indicators(klines_1h: &[Kline], klines_4h: &[Kline]) -> GridIndicators {
    let last_idx = klines_1h.len() - 1;
    let current_price = klines_1h.last().map(|k| k.close).unwrap_or(0.0);

    let rsi = indicators::rsi_at(klines_1h, last_idx, 14);
    let atr = indicators::atr_at(klines_1h, last_idx, 14);
    let bb_width = indicators::bbands_width_at(klines_1h, last_idx, 20, 2.0);

    let ema12 = indicators::ema_at(klines_1h, last_idx, 12);
    let ema26 = indicators::ema_at(klines_1h, last_idx, 26);
    let ema12_prev = indicators::ema_at(klines_1h, last_idx.saturating_sub(5), 12);
    let ema26_prev = indicators::ema_at(klines_1h, last_idx.saturating_sub(5), 26);

    let ema12_trend = if ema12 > ema12_prev { "上升" } else if ema12 < ema12_prev { "下降" } else { "横盘" };
    let ema26_trend = if ema26 > ema26_prev { "上升" } else if ema26 < ema26_prev { "下降" } else { "横盘" };

    let price_high: f64 = klines_1h.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max);
    let price_low: f64 = klines_1h.iter().map(|k| k.low).fold(f64::INFINITY, f64::min);

    let ema_4h = if !klines_4h.is_empty() {
        indicators::ema_at(klines_4h, klines_4h.len() - 1, 26)
    } else {
        0.0
    };

    let last_24: &[Kline] = if klines_1h.len() >= 24 {
        &klines_1h[klines_1h.len() - 24..]
    } else {
        klines_1h
    };
    let high_24: f64 = last_24.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max);
    let low_24: f64 = last_24.iter().map(|k| k.low).fold(f64::INFINITY, f64::min);
    let volatility = if low_24 > 0.0 {
        (high_24 - low_24) / low_24 * 100.0
    } else {
        0.0
    };

    let last_30: &[Kline] = if klines_1h.len() >= 30 {
        &klines_1h[klines_1h.len() - 30..]
    } else {
        klines_1h
    };

    let mut ohlcv_table = String::from("Time,Open,High,Low,Close,Volume\n");
    for k in last_30.iter() {
        let time_str = chrono::DateTime::from_timestamp_millis(k.open_time)
            .map(|dt| dt.format("%m-%d %H:%M").to_string())
            .unwrap_or_else(|| k.open_time.to_string());
        ohlcv_table.push_str(&format!(
            "{},{:.4},{:.4},{:.4},{:.4},{:.4}\n",
            time_str, k.open, k.high, k.low, k.close, k.volume
        ));
    }

    GridIndicators {
        current_price,
        rsi,
        atr,
        bb_width,
        ema12,
        ema26,
        ema12_trend,
        ema26_trend,
        price_high,
        price_low,
        ema_4h,
        volatility,
        ohlcv_table,
    }
}

fn build_user_prompt(template: &str, ind: &GridIndicators, symbol: &str, exchange: &str) -> String {
    template
        .replace("{symbol}", symbol)
        .replace("{exchange}", exchange)
        .replace("{ohlcv_table}", &ind.ohlcv_table)
        .replace("{rsi}", &format!("{:.2}", ind.rsi))
        .replace("{atr}", &format!("{:.4}", ind.atr))
        .replace("{bb_width}", &format!("{:.4}", ind.bb_width))
        .replace("{ema12}", &format!("{:.4}", ind.ema12))
        .replace("{ema12_trend}", ind.ema12_trend)
        .replace("{ema26}", &format!("{:.4}", ind.ema26))
        .replace("{ema26_trend}", ind.ema26_trend)
        .replace("{price_low}", &format!("{:.4}", ind.price_low))
        .replace("{price_high}", &format!("{:.4}", ind.price_high))
        .replace("{current_price}", &format!("{:.4}", ind.current_price))
        .replace("{ema_4h}", &format!("{:.4}", ind.ema_4h))
        .replace("{volatility}", &format!("{:.2}", ind.volatility))
}

struct AiCallResult {
    provider: String,
    used_model: String,
    result: serde_json::Value,
}

async fn call_ai_and_parse(
    state: &Arc<AppState>,
    user_id: &Uuid,
    provider_override: Option<&str>,
    model_override: Option<&str>,
    system_prompt: &str,
    user_prompt: &str,
    error_context: &str,
) -> Result<AiCallResult, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let ai_service = AiService::new(state.config.ai.clone());
    let encryption_key = crypto::derive_key(&state.config.server.encryption_key);
    let user_config = load_user_ai_config(&state.db_pool, user_id, &encryption_key).await;

    if !ai_service.is_configured_with_override(&user_config) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<serde_json::Value>::err(
                "No AI provider configured. Set OPENROUTER_API_KEY, OPENAI_API_KEY, or DEEPSEEK_API_KEY in .env, or configure user-level AI credentials.",
            )),
        ));
    }

    let provider = provider_override
        .unwrap_or_else(|| ai_service.default_provider_with_override(&user_config));

    let (api_key, base_url, model) = ai_service
        .resolve_provider_with_override(provider, model_override, &user_config)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(format!("{}", e))),
            )
        })?;

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "temperature": 0.5,
        "max_tokens": 2000,
    });

    let response = state.http_client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("AI grid {} request failed: {}", error_context, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "Failed to call {} API: {}",
                    provider, e
                ))),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "{} API returned {}: {}",
                provider, status, body_text
            ))),
        ));
    }

    let json: serde_json::Value = response.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to parse {} response: {}",
                provider, e
            ))),
        )
    })?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let used_model = json["model"].as_str().unwrap_or(&model).to_string();

    let content = strip_code_fences(&content);

    let result: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        tracing::error!("Failed to parse AI grid {} JSON: {}", error_context, e);
        tracing::error!("Raw content: {}", content);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "AI returned invalid JSON: {}. Raw response: {}",
                e, content
            ))),
        )
    })?;

    Ok(AiCallResult {
        provider: provider.to_string(),
        used_model,
        result,
    })
}

async fn fetch_klines(
    state: &Arc<AppState>,
    exchange_name: &str,
    symbol: &str,
) -> Result<(Vec<Kline>, Vec<Kline>), (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let exchange_key = super::market::ensure_exchange(state, exchange_name, MarketType::Perpetual).await?;
    let exchange = state.strategy_engine.get_exchange(&exchange_key).unwrap();

    let now_ms = chrono::Utc::now().timestamp_millis();
    let start_1h = now_ms - 200 * 3600 * 1000;
    let start_4h = now_ms - 50 * 4 * 3600 * 1000;

    let klines_1h = match exchange.get_klines_range(symbol, "1h", start_1h, now_ms).await {
        Ok(k) if k.len() >= 30 => k,
        Ok(k) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "Insufficient 1h kline data: got {} candles, need at least 30",
                    k.len()
                ))),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "Failed to fetch 1h klines for {} from {}: {}",
                    symbol, exchange_name, e
                ))),
            ));
        }
    };

    let klines_4h = match exchange.get_klines_range(symbol, "4h", start_4h, now_ms).await {
        Ok(k) => k,
        Err(e) => {
            tracing::warn!("Failed to fetch 4h klines for {}: {}", symbol, e);
            Vec::new()
        }
    };

    Ok((klines_1h, klines_4h))
}

// ── 3.1 POST /api/grid/analyze ──

pub async fn analyze(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<AnalyzeRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    if body.symbol.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("symbol must not be empty")),
        ));
    }

    let user_id = parse_user_id(&auth)?;
    let exchange_name = body.exchange.as_deref().unwrap_or("binance");

    let (klines_1h, klines_4h) = fetch_klines(&state, exchange_name, &body.symbol).await?;
    let ind = compute_grid_indicators(&klines_1h, &klines_4h);

    let system_prompt = match body.system_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => default_grid_system_prompt().to_owned(),
    };

    let user_prompt_template = match body.user_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => DEFAULT_USER_PROMPT_TEMPLATE.to_owned(),
    };
    let user_prompt = build_user_prompt(&user_prompt_template, &ind, &body.symbol, exchange_name);

    let ai = call_ai_and_parse(
        &state,
        &user_id,
        body.provider.as_deref(),
        body.model.as_deref(),
        &system_prompt,
        &user_prompt,
        "analyze",
    )
    .await?;

    tracing::info!(
        "AI grid analysis for {} using {} ({})",
        body.symbol, ai.provider, ai.used_model
    );

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "symbol": body.symbol,
        "exchange": exchange_name,
        "provider": ai.provider,
        "model": ai.used_model,
        "analysis": ai.result,
        "indicators": {
            "rsi": ind.rsi,
            "atr": ind.atr,
            "bb_width": ind.bb_width,
            "ema12": ind.ema12,
            "ema26": ind.ema26,
            "current_price": ind.current_price,
            "volatility": ind.volatility,
        }
    }))))
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

    let user_id = parse_user_id(&auth)?;

    let upper_price = body.upper_price.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("upper_price is required")),
        )
    })?;

    let lower_price = body.lower_price.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("lower_price is required")),
        )
    })?;

    if upper_price <= lower_price {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err(
                "upper_price must be greater than lower_price",
            )),
        ));
    }

    let grid_count = body.grid_count.unwrap_or(20);
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
            market_regime, ai_analysis, system_prompt, user_prompt,
            dynamic_adjust, adjust_interval_secs
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
        RETURNING *"#,
    )
    .bind(user_id)
    .bind(&body.name)
    .bind(&body.symbol)
    .bind(&exchange)
    .bind(StrategyStatus::Draft)
    .bind(upper_price)
    .bind(lower_price)
    .bind(grid_count)
    .bind(grid_profit_pct)
    .bind(quantity_per_grid)
    .bind(leverage)
    .bind(&body.market_regime)
    .bind(&body.ai_analysis)
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

    let filled_levels: Vec<i32> = sqlx::query_scalar(
        r#"SELECT DISTINCT grid_level FROM qd_grid_trades WHERE bot_id = $1"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    let filled_set: std::collections::HashSet<i32> = filled_levels.into_iter().collect();

    let grid_spacing = if bot.grid_count > 1 {
        (bot.upper_price - bot.lower_price) / bot.grid_count as f64
    } else {
        0.0
    };

    let mut grid_levels = Vec::new();
    for i in 0..=bot.grid_count {
        let price = bot.lower_price + grid_spacing * i as f64;
        grid_levels.push(serde_json::json!({
            "level": i,
            "price": price,
            "filled": filled_set.contains(&i),
        }));
    }

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
        StrategyStatus::Stopped => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(
                    "Cannot start a stopped bot. Create a new one.",
                )),
            ));
        }
        _ => {}
    }

    let updated = sqlx::query_as::<_, GridBot>(
        r#"UPDATE qd_grid_bots SET status = $2, started_at = NOW(), updated_at = NOW()
           WHERE id = $1 RETURNING *"#,
    )
    .bind(id)
    .bind(StrategyStatus::Running)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to start bot: {}", e
            ))),
        )
    })?;

    Ok(Json(ApiResponse::ok(serde_json::json!({ "bot": updated }))))
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

    let updated = sqlx::query_as::<_, GridBot>(
        r#"UPDATE qd_grid_bots SET status = $2, stopped_at = NOW(), updated_at = NOW()
           WHERE id = $1 RETURNING *"#,
    )
    .bind(id)
    .bind(StrategyStatus::Stopped)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to stop bot: {}", e
            ))),
        )
    })?;

    Ok(Json(ApiResponse::ok(serde_json::json!({ "bot": updated }))))
}

// ── 3.7 DELETE /api/grid/{id}/delete ──

pub async fn delete_bot(
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

    match bot {
        Some(b) => {
            if b.status == StrategyStatus::Running {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<serde_json::Value>::err(
                        "Cannot delete a running bot. Stop it first.",
                    )),
                ));
            }
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<serde_json::Value>::err("Grid bot not found")),
            ));
        }
    }

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

    Ok(Json(ApiResponse::ok_with_message(
        serde_json::json!({ "deleted": true }),
        "Grid bot deleted successfully",
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

    let total_pages = (total.0 + page_size - 1) / page_size;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": trades,
        "total": total.0,
        "page": page,
        "page_size": page_size,
        "total_pages": total_pages,
    }))))
}

// ── 3.9 POST /api/grid/{id}/reanalyze ──

pub async fn reanalyze(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReanalyzeRequest>,
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

    if bot.status == StrategyStatus::Running {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err(
                "Cannot reanalyze a running bot. Pause or stop it first.",
            )),
        ));
    }

    let (klines_1h, klines_4h) = fetch_klines(&state, &bot.exchange, &bot.symbol).await?;
    let ind = compute_grid_indicators(&klines_1h, &klines_4h);

    let system_prompt = match body.system_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => bot.system_prompt.as_deref().unwrap_or_else(|| default_grid_system_prompt()).to_owned(),
    };

    let user_prompt_template = match body.user_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => bot.user_prompt.as_deref().unwrap_or(DEFAULT_USER_PROMPT_TEMPLATE).to_owned(),
    };
    let user_prompt = build_user_prompt(&user_prompt_template, &ind, &bot.symbol, &bot.exchange);

    let ai = call_ai_and_parse(
        &state,
        &user_id,
        body.provider.as_deref(),
        body.model.as_deref(),
        &system_prompt,
        &user_prompt,
        "reanalyze",
    )
    .await?;

    let new_market_regime = ai.result["market_regime"].as_str().unwrap_or("ranging").to_string();
    let new_upper_price = ai.result["upper_price"].as_f64().unwrap_or(bot.upper_price);
    let new_lower_price = ai.result["lower_price"].as_f64().unwrap_or(bot.lower_price);
    let new_grid_count = ai.result["grid_count"].as_i64().unwrap_or(bot.grid_count as i64) as i32;
    let new_grid_profit_pct = ai.result["grid_profit_pct"].as_f64().unwrap_or(bot.grid_profit_pct);
    let new_quantity_per_grid = ai.result["quantity_per_grid"].as_f64().unwrap_or(bot.quantity_per_grid);
    let new_leverage = ai.result["leverage"].as_i64().unwrap_or(bot.leverage as i64) as i32;
    let new_analysis = ai.result["analysis"].as_str().unwrap_or("").to_string();

    let updated = sqlx::query_as::<_, GridBot>(
        r#"UPDATE qd_grid_bots SET
            upper_price = $1, lower_price = $2, grid_count = $3,
            grid_profit_pct = $4, quantity_per_grid = $5, leverage = $6,
            market_regime = $7, ai_analysis = $8, last_adjusted_at = NOW(),
            updated_at = NOW()
           WHERE id = $9 RETURNING *"#,
    )
    .bind(new_upper_price)
    .bind(new_lower_price)
    .bind(new_grid_count)
    .bind(new_grid_profit_pct)
    .bind(new_quantity_per_grid)
    .bind(new_leverage)
    .bind(&new_market_regime)
    .bind(&new_analysis)
    .bind(id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "Failed to update bot: {}", e
            ))),
        )
    })?;

    tracing::info!(
        "Grid bot {} reanalyzed using {} ({})",
        id, ai.provider, ai.used_model
    );

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "bot": updated,
        "analysis": ai.result,
    }))))
}
