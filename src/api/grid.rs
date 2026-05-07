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
use crate::indicators;
use crate::models::*;
use crate::services::ai::{AiService, AiUserConfig};
use crate::utils::crypto;

// ── Request / Response Types ──

#[derive(Debug, Deserialize)]
pub struct AnalyzeRequest {
    pub bot_id: Uuid,
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
    pub grid_count: Option<i32>,
    pub grid_profit_pct: Option<f64>,
    pub quantity_per_grid: Option<f64>,
    pub leverage: Option<i32>,
    pub dynamic_adjust: Option<bool>,
    pub adjust_interval_secs: Option<i32>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
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
    r#"你是一位专业的加密货币量化交易分析师，精通合约网格交易策略。你的职责是分析市场数据、判断市场状态、生成最优网格参数，并给出可执行的交易操作指令。

## 核心参数
- 交易对由用户提供
- 网格层数由用户提供（默认 50）
- 总投资额由用户提供（USDT）
- 杠杆倍数由用户提供
- 价格分布采用高斯分布（中间密、两端疏）

## 市场状态判断规则

### 震荡市场（适合网格交易）
- BBands Width < 3%（布林带收缩，价格在通道内运行）
- EMA20 与 EMA50 距离 < 1%（均线粘合，无明确方向）
- 价格在布林带中轨 ±1% 附近
- ADX < 25（趋势不明显）
- **操作**: 正常运行网格，place_buy_limit / place_sell_limit

### 趋势市场（暂停网格）
- BBands Width > 4%（布林带扩张，趋势启动）
- EMA20 与 EMA50 距离 > 2%（均线发散，方向明确）
- 价格持续突破布林带上轨或下轨（连续 3 根以上）
- ADX > 30（趋势强劲）
- **操作**: pause_grid，等待回归震荡后再 resume_grid

### 高波动市场（谨慎运行）
- ATR 异常放大（当前 ATR > 20 日 ATR 均值的 2 倍）
- 价格在短时间内剧烈波动（1h K 线实体 > ATR 的 1.5 倍）
- BBands Width 突然扩张（5 根 bar 内增幅 > 50%）
- **操作**: 可继续运行但减小仓位（quantity_per_grid × 0.5），或 pause_grid

## 网格参数计算规则

### 上下界确定
- 上界：近期阻力位（近期高点、BBands 上轨、整数关口）取最低值
- 下界：近期支撑位（近期低点、BBands 下轨、整数关口）取最高值
- 网格区间应覆盖当前价格 ±2 个标准差（约 95% 置信区间）
- 区间宽度 = 上界 - 下界，应 >= ATR × 10（确保足够的交易空间）

### 高斯分布网格
- 网格价格按高斯分布排列：中间密度高、两端密度低
- 使用当前价格为均值 μ，区间宽度 / 4 为标准差 σ
- 每个网格价格 = μ + σ × Φ⁻¹(p)，其中 p 按网格序号均匀分布
- 这样在价格密集区域（中间）有更多网格，捕捉更多交易机会

### 每格利润率
- 基础利润率 = (网格间距 / 网格价格) × 100%
- 考虑手续费（taker 0.05% × 2 = 0.1%），实际利润率应 > 0.3%
- 建议每格利润率 0.3% - 2.0%，波动率越高利润率可越大

### 每格数量
- 每格数量(USDT) = 总投资额 / 有效网格数
- 有效网格数 ≈ grid_count × 0.6（高斯分布下约 60% 的网格在 1σ 内）
- 实际下单数量 = 每格数量 / 杠杆倍数 / 当前价格（换算为币数）

## 可执行操作指令
- `place_buy_limit` — 在指定价格挂买单
- `place_sell_limit` — 在指定价格挂卖单
- `cancel_order` — 取消指定订单
- `cancel_all_orders` — 取消所有挂单
- `pause_grid` — 暂停网格（趋势市场时）
- `resume_grid` — 恢复网格（回归震荡时）
- `adjust_grid` — 调整网格上下界
- `hold` — 保持当前状态不操作

## 风控规则
1. 单次最大持仓不超过总投资的 30%
2. 网格区间内最大亏损不超过总投资的 15%
3. 当价格突破网格区间时，立即 cancel_all_orders 并 pause_grid
4. 当连续 3 次交易亏损时，减小仓位至 50%
5. 杠杆使用不超过 10 倍，高波动市场不超过 3 倍

## 输出格式（严格 JSON，不要 markdown 代码块）
{
  "market_regime": "ranging|trending_up|trending_down|volatile",
  "confidence": 0.0-1.0,
  "recommended_action": "run_grid|pause_grid|reduce_position|adjust_grid",
  "action_reason": "推荐操作的理由（50字以内）",
  "upper_price": 数字（网格上界）,
  "lower_price": 数字（网格下界）,
  "grid_count": 数字（网格层数）,
  "grid_profit_pct": 数字（每格利润率%）,
  "quantity_per_grid": 数字（每格数量，USDT）,
  "leverage": 数字（杠杆倍数）,
  "grid_levels": [
    { "level": 1, "price": 数字, "side": "buy", "quantity_usdt": 数字 },
    { "level": 2, "price": 数字, "side": "buy", "quantity_usdt": 数字 },
    ...
    { "level": N, "price": 数字, "side": "sell", "quantity_usdt": 数字 }
  ],
  "analysis": "详细分析说明（300字以内）",
  "risk_warning": "风险提示（100字以内）"
}"#
}

const DEFAULT_USER_PROMPT_TEMPLATE: &str = r#"请分析以下市场数据并生成网格交易参数：

## 交易对
{symbol} ({exchange}) - 永续合约

## 近期K线数据（最近30根1h）
{ohlcv_table}

## 关键指标（除特别标注外均为 1h 周期）
- RSI(14, 1h): {rsi}
- ATR(14, 1h): {atr}
- BBands Width(1h): {bb_width}
- EMA(12, 1h): {ema12} (方向: {ema12_trend})
- EMA(26, 1h): {ema26} (方向: {ema26_trend})
- 价格区间(1h): {price_low} - {price_high}
- 当前价格: {current_price}
- EMA(26, 4h): {ema_4h}
- 资金费率: {funding_rate}%
- 24h 波动率: {volatility}%

请生成适合当前市场状态的网格交易参数。"#;

// ── Shared indicator computation ──

struct GridIndicators {
    current_price: f64,
    rsi: f64,
    atr: f64,
    atr_pct: f64,
    bb_width: f64,
    bb_upper: f64,
    bb_middle: f64,
    bb_lower: f64,
    ema12: f64,
    ema20: f64,
    ema26: f64,
    ema50: f64,
    ema12_trend: &'static str,
    ema20_trend: &'static str,
    ema26_trend: &'static str,
    ema50_trend: &'static str,
    price_high: f64,
    price_low: f64,
    ema_4h: f64,
    volatility: f64,
    change_1h: f64,
    change_4h: f64,
    change_24h: f64,
    macd: f64,
    macd_signal: f64,
    macd_histogram: f64,
    adx: f64,
    funding_rate: f64,
    ohlcv_table: String,
}

async fn compute_grid_indicators(
    klines_1h: &[Kline],
    klines_4h: &[Kline],
    exchange: &dyn crate::exchange::Exchange,
    symbol: &str,
) -> GridIndicators {
    let last_idx = klines_1h.len() - 1;
    let current_price = klines_1h.last().map(|k| k.close).unwrap_or(0.0);

    let rsi = indicators::rsi_at(klines_1h, last_idx, 14);
    let atr = indicators::atr_at(klines_1h, last_idx, 14);
    let atr_pct = if current_price > 0.0 { atr / current_price * 100.0 } else { 0.0 };
    let bb_width = indicators::bbands_width_at(klines_1h, last_idx, 20, 2.0);
    let (bb_upper, bb_middle, bb_lower) = indicators::bbands_at(klines_1h, last_idx, 20, 2.0);

    let ema12 = indicators::ema_at(klines_1h, last_idx, 12);
    let ema20 = indicators::ema_at(klines_1h, last_idx, 20);
    let ema26 = indicators::ema_at(klines_1h, last_idx, 26);
    let ema50 = if klines_1h.len() >= 50 {
        indicators::ema_at(klines_1h, last_idx, 50)
    } else {
        0.0
    };

    let lookback = 5.min(last_idx);
    let ema12_prev = indicators::ema_at(klines_1h, last_idx.saturating_sub(lookback), 12);
    let ema20_prev = indicators::ema_at(klines_1h, last_idx.saturating_sub(lookback), 20);
    let ema26_prev = indicators::ema_at(klines_1h, last_idx.saturating_sub(lookback), 26);
    let ema50_prev = if klines_1h.len() >= 50 + lookback {
        indicators::ema_at(klines_1h, last_idx.saturating_sub(lookback), 50)
    } else {
        ema50
    };

    let ema12_trend = if ema12 > ema12_prev { "上升" } else if ema12 < ema12_prev { "下降" } else { "横盘" };
    let ema20_trend = if ema20 > ema20_prev { "上升" } else if ema20 < ema20_prev { "下降" } else { "横盘" };
    let ema26_trend = if ema26 > ema26_prev { "上升" } else if ema26 < ema26_prev { "下降" } else { "横盘" };
    let ema50_trend = if ema50 > ema50_prev { "上升" } else if ema50 < ema50_prev { "下降" } else { "横盘" };

    let price_high: f64 = klines_1h.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max);
    let price_low: f64 = klines_1h.iter().map(|k| k.low).fold(f64::INFINITY, f64::min);

    let ema_4h = if !klines_4h.is_empty() {
        indicators::ema_at(klines_4h, klines_4h.len() - 1, 26)
    } else {
        0.0
    };

    // 涨跌幅计算
    let change_1h = if last_idx >= 1 && klines_1h[last_idx - 1].close > 0.0 {
        (current_price - klines_1h[last_idx - 1].close) / klines_1h[last_idx - 1].close * 100.0
    } else { 0.0 };

    let change_4h = if last_idx >= 4 && klines_1h[last_idx - 4].close > 0.0 {
        (current_price - klines_1h[last_idx - 4].close) / klines_1h[last_idx - 4].close * 100.0
    } else { 0.0 };

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
    let change_24h = if last_24.first().map(|k| k.close).unwrap_or(0.0) > 0.0 {
        (current_price - last_24.first().unwrap().close) / last_24.first().unwrap().close * 100.0
    } else { 0.0 };

    // MACD
    let macd = indicators::macd_at(klines_1h, last_idx, 12, 26);
    let macd_signal = indicators::macd_signal_at(klines_1h, last_idx, 12, 26, 9);
    let macd_histogram = indicators::macd_histogram_at(klines_1h, last_idx, 12, 26, 9);

    // ADX
    let adx = indicators::adx_at(klines_1h, last_idx, 14);

    // Funding rate (perpetual only, best-effort)
    let funding_rate = exchange
        .get_funding_rate(symbol)
        .await
        .map(|fr| fr.rate)
        .unwrap_or(0.0);

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
        atr_pct,
        bb_width,
        bb_upper,
        bb_middle,
        bb_lower,
        ema12,
        ema20,
        ema26,
        ema50,
        ema12_trend,
        ema20_trend,
        ema26_trend,
        ema50_trend,
        price_high,
        price_low,
        ema_4h,
        volatility,
        change_1h,
        change_4h,
        change_24h,
        macd,
        macd_signal,
        macd_histogram,
        adx,
        funding_rate,
        ohlcv_table,
    }
}

fn build_user_prompt(template: &str, ind: &GridIndicators, symbol: &str, exchange: &str) -> String {
    template
        .replace("{symbol}", symbol)
        .replace("{exchange}", exchange)
        .replace("{current_time}", &chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
        .replace("{current_price}", &format!("{:.2}", ind.current_price))
        .replace("{change_1h}", &format!("{:+.2}%", ind.change_1h))
        .replace("{change_4h}", &format!("{:+.2}%", ind.change_4h))
        .replace("{change_24h}", &format!("{:+.2}%", ind.change_24h))
        .replace("{rsi}", &format!("{:.2}", ind.rsi))
        .replace("{atr}", &format!("{:.4}", ind.atr))
        .replace("{atr_pct}", &format!("{:.2}%", ind.atr_pct))
        .replace("{bb_upper}", &format!("{:.2}", ind.bb_upper))
        .replace("{bb_middle}", &format!("{:.2}", ind.bb_middle))
        .replace("{bb_lower}", &format!("{:.2}", ind.bb_lower))
        .replace("{bb_width}", &format!("{:.2}%", ind.bb_width))
        .replace("{ema12}", &format!("{:.4}", ind.ema12))
        .replace("{ema12_trend}", ind.ema12_trend)
        .replace("{ema20}", &format!("{:.4}", ind.ema20))
        .replace("{ema20_trend}", ind.ema20_trend)
        .replace("{ema26}", &format!("{:.4}", ind.ema26))
        .replace("{ema26_trend}", ind.ema26_trend)
        .replace("{ema50}", &format!("{:.4}", ind.ema50))
        .replace("{ema50_trend}", ind.ema50_trend)
        .replace("{price_high}", &format!("{:.4}", ind.price_high))
        .replace("{price_low}", &format!("{:.4}", ind.price_low))
        .replace("{ema_4h}", &format!("{:.4}", ind.ema_4h))
        .replace("{volatility}", &format!("{:.2}", ind.volatility))
        .replace("{macd}", &format!("{:.4}", ind.macd))
        .replace("{macd_signal}", &format!("{:.4}", ind.macd_signal))
        .replace("{macd_histogram}", &format!("{:.4}", ind.macd_histogram))
        .replace("{adx}", &format!("{:.2}", ind.adx))
        .replace("{funding_rate}", &format!("{:.6}", ind.funding_rate * 100.0))
        .replace("{ohlcv_table}", &ind.ohlcv_table)
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
        "response_format": { "type": "json_object" },
        "temperature": 0.5,
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

    tracing::debug!("AI grid {} raw response: {}", error_context, serde_json::ser::to_string(&json).unwrap_or_default());

    // json_object mode: content should be valid JSON directly
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if content.is_empty() {
        tracing::error!(
            "AI grid {} returned empty content. Full response: {}",
            error_context,
            serde_json::ser::to_string(&json).unwrap_or_default()
        );
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(
                "AI returned empty response. The provider may not support json_object mode."
            )),
        ));
    }

    let used_model = json["model"].as_str().unwrap_or(&model).to_string();

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
    let exchange = state.exchange_registry.get(&exchange_key).unwrap();

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
    let user_id = parse_user_id(&auth)?;

    // 从数据库加载 bot
    let bot = sqlx::query_as::<_, GridBot>(
        r#"SELECT * FROM qd_grid_bots WHERE id = $1 AND user_id = $2"#,
    )
    .bind(body.bot_id)
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

    let symbol = &bot.symbol;
    let exchange_name = &bot.exchange;

    let (klines_1h, klines_4h) = fetch_klines(&state, exchange_name, symbol).await?;
    let exchange_key = super::market::ensure_exchange(&state, exchange_name, MarketType::Perpetual).await?;
    let exchange = state.exchange_registry.get(&exchange_key).unwrap();
    let ind = compute_grid_indicators(&klines_1h, &klines_4h, exchange.as_ref(), symbol).await;

    let system_prompt = match body.system_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => bot.system_prompt.as_deref().unwrap_or_else(|| default_grid_system_prompt()).to_owned(),
    };

    let user_prompt_template = match body.user_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => bot.user_prompt.as_deref().unwrap_or(DEFAULT_USER_PROMPT_TEMPLATE).to_owned(),
    };
    let user_prompt = build_user_prompt(&user_prompt_template, &ind, symbol, exchange_name);

    // 插入 pending 分析日志
    let log_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO qd_grid_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status)
           VALUES ($1, $2, $3, $4, 'pending') RETURNING id"#,
    )
    .bind(body.bot_id)
    .bind("analyze")
    .bind(&system_prompt)
    .bind(&user_prompt)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(Uuid::nil());

    let ai_result = call_ai_and_parse(
        &state,
        &user_id,
        body.provider.as_deref(),
        body.model.as_deref(),
        &system_prompt,
        &user_prompt,
        "analyze",
    ).await;

    match ai_result {
        Ok(ai) => {
            // 更新日志为 completed
            let _ = sqlx::query(
                r#"UPDATE qd_grid_analysis_logs SET status = 'completed', result = $1, completed_at = NOW() WHERE id = $2"#,
            )
            .bind(&ai.result)
            .bind(log_id)
            .execute(&state.db_pool)
            .await;

            tracing::info!(
                "AI grid analysis for {} using {} ({})",
                symbol, ai.provider, ai.used_model
            );

            // 从 LLM 返回结果中提取参数
            let new_upper_price = ai.result["upper_price"].as_f64().unwrap_or(0.0);
            let new_lower_price = ai.result["lower_price"].as_f64().unwrap_or(0.0);
            let new_grid_count = ai.result["grid_count"].as_i64().unwrap_or(0) as i32;
            let new_grid_profit_pct = ai.result["grid_profit_pct"].as_f64().unwrap_or(0.5);
            let new_quantity_per_grid = ai.result["quantity_per_grid"].as_f64().unwrap_or(10.0);
            let new_leverage = ai.result["leverage"].as_i64().unwrap_or(1) as i32;
            let new_market_regime = ai.result["market_regime"].as_str().unwrap_or("ranging").to_string();
            let new_analysis = ai.result["analysis"].as_str().unwrap_or("").to_string();

            // 更新 bot 参数
            let updated = sqlx::query_as::<_, GridBot>(
                r#"UPDATE qd_grid_bots SET
                    upper_price = $1, lower_price = $2, grid_count = $3,
                    grid_profit_pct = $4, quantity_per_grid = $5, leverage = $6,
                    market_regime = $7, ai_analysis = $8,
                    system_prompt = $9, user_prompt = $10,
                    updated_at = NOW()
                   WHERE id = $11 RETURNING *"#,
            )
            .bind(new_upper_price)
            .bind(new_lower_price)
            .bind(new_grid_count)
            .bind(new_grid_profit_pct)
            .bind(new_quantity_per_grid)
            .bind(new_leverage)
            .bind(&new_market_regime)
            .bind(&new_analysis)
            .bind(&system_prompt)
            .bind(&user_prompt)
            .bind(body.bot_id)
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

            Ok(Json(ApiResponse::ok(serde_json::json!({
                "bot": updated,
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
        Err(e) => {
            // 更新日志为 failed
            let error_msg = match &e {
                (StatusCode::SERVICE_UNAVAILABLE, json) => json.0.error.clone().unwrap_or_default(),
                (_, json) => json.0.error.clone().unwrap_or_default(),
            };
            let _ = sqlx::query(
                r#"UPDATE qd_grid_analysis_logs SET status = 'failed', error = $1, completed_at = NOW() WHERE id = $2"#,
            )
            .bind(&error_msg)
            .bind(log_id)
            .execute(&state.db_pool)
            .await;

            Err(e)
        }
    }
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
    .bind(&None::<String>)
    .bind(&None::<String>)
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

    // 计算每个层级的成交量
    let level_quantities: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
        r#"SELECT grid_level, SUM(quantity) FROM qd_grid_trades WHERE bot_id = $1 GROUP BY grid_level"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let grid_spacing = if bot.grid_count > 1 {
        (bot.upper_price - bot.lower_price) / bot.grid_count as f64
    } else {
        0.0
    };

    let mut grid_levels = Vec::new();
    for i in 0..=bot.grid_count {
        let price = bot.lower_price + grid_spacing * i as f64;
        let quantity = level_quantities.get(&i).copied().unwrap_or(0.0);
        grid_levels.push(serde_json::json!({
            "level": i,
            "price": price,
            "filled": filled_set.contains(&i),
            "quantity": quantity,
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

    // 如果参数无效，先进行 AI 分析
    let bot = if bot.upper_price <= 0.0 || bot.lower_price <= 0.0 || bot.grid_count <= 0 {
        tracing::info!(bot_id = %id, "Bot has no valid parameters, running initial analysis");

        // 获取市场数据
        let (klines_1h, klines_4h) = fetch_klines(&state, &bot.exchange, &bot.symbol).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::err(format!("Failed to fetch klines: {}", e.1 .0.error.unwrap_or_default())))))?;
        let exchange_key = super::market::ensure_exchange(&state, &bot.exchange, MarketType::Perpetual).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::err(format!("Failed to init exchange: {}", e.1 .0.error.unwrap_or_default())))))?;
        let exchange = state.exchange_registry.get(&exchange_key).unwrap();
        let ind = compute_grid_indicators(&klines_1h, &klines_4h, exchange.as_ref(), &bot.symbol).await;

        let system_prompt = bot.system_prompt.as_deref().unwrap_or_else(|| default_grid_system_prompt()).to_owned();
        let user_prompt_template = bot.user_prompt.as_deref().unwrap_or(DEFAULT_USER_PROMPT_TEMPLATE).to_owned();
        let user_prompt = build_user_prompt(&user_prompt_template, &ind, &bot.symbol, &bot.exchange);

        // 插入 pending 分析日志
        let log_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO qd_grid_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status)
               VALUES ($1, $2, $3, $4, 'pending') RETURNING id"#,
        )
        .bind(id)
        .bind("initial")
        .bind(&system_prompt)
        .bind(&user_prompt)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(Uuid::nil());

        // 调用 AI
        let ai_result = call_ai_and_parse(
            &state,
            &user_id,
            None, None,
            &system_prompt,
            &user_prompt,
            "initial",
        ).await;

        match ai_result {
            Ok(ai) => {
                // 更新日志为 completed
                let _ = sqlx::query(
                    r#"UPDATE qd_grid_analysis_logs SET status = 'completed', result = $1, completed_at = NOW() WHERE id = $2"#,
                )
                .bind(&ai.result)
                .bind(log_id)
                .execute(&state.db_pool)
                .await;

                let new_upper_price = ai.result["upper_price"].as_f64().unwrap_or(0.0);
                let new_lower_price = ai.result["lower_price"].as_f64().unwrap_or(0.0);
                let new_grid_count = ai.result["grid_count"].as_i64().unwrap_or(0) as i32;
                let new_grid_profit_pct = ai.result["grid_profit_pct"].as_f64().unwrap_or(0.5);
                let new_quantity_per_grid = ai.result["quantity_per_grid"].as_f64().unwrap_or(10.0);
                let new_leverage = ai.result["leverage"].as_i64().unwrap_or(1) as i32;
                let new_market_regime = ai.result["market_regime"].as_str().unwrap_or("ranging").to_string();
                let new_analysis = ai.result["analysis"].as_str().unwrap_or("").to_string();

                // 更新 bot 参数
                let updated = sqlx::query_as::<_, GridBot>(
                    r#"UPDATE qd_grid_bots SET
                        upper_price = $1, lower_price = $2, grid_count = $3,
                        grid_profit_pct = $4, quantity_per_grid = $5, leverage = $6,
                        market_regime = $7, ai_analysis = $8,
                        system_prompt = $9, user_prompt = $10,
                        updated_at = NOW()
                       WHERE id = $11 RETURNING *"#,
                )
                .bind(new_upper_price)
                .bind(new_lower_price)
                .bind(new_grid_count)
                .bind(new_grid_profit_pct)
                .bind(new_quantity_per_grid)
                .bind(new_leverage)
                .bind(&new_market_regime)
                .bind(&new_analysis)
                .bind(&system_prompt)
                .bind(&user_prompt)
                .bind(id)
                .fetch_one(&state.db_pool)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiResponse::<serde_json::Value>::err(format!("Failed to update bot: {}", e))),
                    )
                })?;

                tracing::info!(bot_id = %id, "Initial analysis completed");
                updated
            }
            Err(e) => {
                // 更新日志为 failed
                let error_msg = match &e {
                    (StatusCode::SERVICE_UNAVAILABLE, json) => json.0.error.clone().unwrap_or_default(),
                    (_, json) => json.0.error.clone().unwrap_or_default(),
                };
                let _ = sqlx::query(
                    r#"UPDATE qd_grid_analysis_logs SET status = 'failed', error = $1, completed_at = NOW() WHERE id = $2"#,
                )
                .bind(&error_msg)
                .bind(log_id)
                .execute(&state.db_pool)
                .await;

                return Err(e);
            }
        }
    } else {
        bot
    };

    // 通过 GridEngine 启动 bot
    if let Some(ref grid_cmd_tx) = state.grid_cmd_tx {
        if let Err(e) = grid_cmd_tx.send(crate::bot::semi_automatic_grid::types::GridCommand::StartBot { bot_id: id }).await {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Failed to start grid engine: {}", e))),
            ));
        }
    }

    // 同步更新 DB 状态，确保 API 返回时前端能看到正确状态
    let _ = sqlx::query(
        r#"UPDATE qd_grid_bots SET status = $2, started_at = NOW(), updated_at = NOW() WHERE id = $1"#,
    )
    .bind(id)
    .bind(StrategyStatus::Running)
    .execute(&state.db_pool)
    .await;

    let updated = sqlx::query_as::<_, GridBot>(
        "SELECT * FROM qd_grid_bots WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(bot);

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

    // 通过 GridEngine 停止 bot
    if let Some(ref grid_cmd_tx) = state.grid_cmd_tx {
        if let Err(e) = grid_cmd_tx.send(crate::bot::semi_automatic_grid::types::GridCommand::StopBot { bot_id: id }).await {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::err(format!("Failed to stop grid engine: {}", e))),
            ));
        }
    }

    // 同步更新 DB 状态
    let _ = sqlx::query(
        r#"UPDATE qd_grid_bots SET status = $2, stopped_at = NOW(), updated_at = NOW() WHERE id = $1"#,
    )
    .bind(id)
    .bind(StrategyStatus::Stopped)
    .execute(&state.db_pool)
    .await;

    let updated = sqlx::query_as::<_, GridBot>(
        "SELECT * FROM qd_grid_bots WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(bot);

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

    let was_running = bot.status == StrategyStatus::Running;

    let (klines_1h, klines_4h) = fetch_klines(&state, &bot.exchange, &bot.symbol).await?;
    let exchange_key = super::market::ensure_exchange(&state, &bot.exchange, MarketType::Perpetual).await?;
    let exchange = state.exchange_registry.get(&exchange_key).unwrap();
    let ind = compute_grid_indicators(&klines_1h, &klines_4h, exchange.as_ref(), &bot.symbol).await;

    let system_prompt = match body.system_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => bot.system_prompt.as_deref().unwrap_or_else(|| default_grid_system_prompt()).to_owned(),
    };

    let user_prompt_template = match body.user_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => bot.user_prompt.as_deref().unwrap_or(DEFAULT_USER_PROMPT_TEMPLATE).to_owned(),
    };
    let user_prompt = build_user_prompt(&user_prompt_template, &ind, &bot.symbol, &bot.exchange);

    // 插入 pending 分析日志
    let log_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO qd_grid_analysis_logs (bot_id, analysis_type, system_prompt, user_prompt, status)
           VALUES ($1, $2, $3, $4, 'pending') RETURNING id"#,
    )
    .bind(id)
    .bind("reanalyze")
    .bind(&system_prompt)
    .bind(&user_prompt)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(Uuid::nil());

    let ai_result = call_ai_and_parse(
        &state,
        &user_id,
        body.provider.as_deref(),
        body.model.as_deref(),
        &system_prompt,
        &user_prompt,
        "reanalyze",
    ).await;

    match ai_result {
        Ok(ai) => {
            // 更新日志为 completed
            let _ = sqlx::query(
                r#"UPDATE qd_grid_analysis_logs SET status = 'completed', result = $1, completed_at = NOW() WHERE id = $2"#,
            )
            .bind(&ai.result)
            .bind(log_id)
            .execute(&state.db_pool)
            .await;

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

            // 如果 bot 之前是 running 状态，发送 AdjustGrid 命令
            if was_running {
                if let Some(ref grid_cmd_tx) = state.grid_cmd_tx {
                    if let Err(e) = grid_cmd_tx.send(crate::bot::semi_automatic_grid::types::GridCommand::AdjustGrid { bot_id: id }).await {
                        tracing::warn!("Failed to send AdjustGrid command for bot {}: {}", id, e);
                    }
                }
            }

            Ok(Json(ApiResponse::ok(serde_json::json!({
                "bot": updated,
                "analysis": ai.result,
            }))))
        }
        Err(e) => {
            // 更新日志为 failed
            let error_msg = match &e {
                (StatusCode::SERVICE_UNAVAILABLE, json) => json.0.error.clone().unwrap_or_default(),
                (_, json) => json.0.error.clone().unwrap_or_default(),
            };
            let _ = sqlx::query(
                r#"UPDATE qd_grid_analysis_logs SET status = 'failed', error = $1, completed_at = NOW() WHERE id = $2"#,
            )
            .bind(&error_msg)
            .bind(log_id)
            .execute(&state.db_pool)
            .await;

            Err(e)
        }
    }
}

// ── Paper Trading ──

/// GET /api/grid/paper/status — 获取 paper 交易状态
pub async fn paper_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let paper = match &state.paper_executor {
        Some(p) => p,
        None => {
            return Ok(Json(ApiResponse::ok(serde_json::json!({
                "enabled": false,
                "pending_count": 0,
            }))));
        }
    };

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "enabled": paper.is_enabled(),
        "pending_count": paper.pending_count().await,
    }))))
}

/// POST /api/grid/paper/enable — 启用 paper 交易
pub async fn paper_enable(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    match &state.paper_executor {
        Some(paper) => {
            paper.enable();
            Ok(Json(ApiResponse::ok_with_message(
                serde_json::json!({ "enabled": true }),
                "Paper trading enabled",
            )))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("Paper executor not available")),
        )),
    }
}

/// POST /api/grid/paper/disable — 禁用 paper 交易
pub async fn paper_disable(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    match &state.paper_executor {
        Some(paper) => {
            paper.disable().await;
            Ok(Json(ApiResponse::ok_with_message(
                serde_json::json!({ "enabled": false }),
                "Paper trading disabled",
            )))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::err("Paper executor not available")),
        )),
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
