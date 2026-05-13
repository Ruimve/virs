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
use crate::bot::semi_automatic_grid::types::DEFAULT_USER_PROMPT_TEMPLATE;
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
    r#"你是一位专业的加密货币量化网格交易分析师。基于实时数据、当前网格与仓位，判断市场状态并给出严谨的JSON操作指令。

## 多周期框架
- 主周期：1h（趋势震荡基准、网格区间计算）
- 快周期：15m（突破确认、假突破过滤）
- 慢周期：4h（大趋势验证，避免逆势）
信号一致提置信度，冲突时降置信度并保守操作。

## 指标公式（各周期独立计算）
- 布林带(20,2)：中轨=SMA20，带宽=(上-下)/中轨*100%
- 均线距离：abs(EMA20-EMA50)/EMA50*100%
- ADX(14)、ATR(14)
- K线实体=abs(收盘-开盘)

## 市场状态判断

### ranging（震荡）
1h全部满足：带宽<3%，均线距离<1%，价格距中轨<1%，ADX<25。
若4h强趋势（ADX>30，均线发散）但1h震荡，标记“高不确定性震荡”，conf≤0.6，仓位×0.7。

### trending_up / trending_down（趋势）
1h满足：带宽>4%，均线距离>2%，连续3根收盘出轨，ADX>30。
方向：EMA20>EMA50且收盘>上轨→up；反之下。
4h同向→conf≥0.85，反向则conf≤0.5仅暂停不反手。
趋势市场**必须暂停网格**。

### volatile（高波动）
1h任一：ATR>ATR_SMA20*2，实体>ATR*1.5，5根内带宽增幅>50%。
15m ATR同步放大确认。杠杆≤3倍，每格数量减半，conf 0.55~0.7。

### transition（过渡）
指标多周期严重冲突且无法归类，conf≤0.4，操作hold或reduce_position。

## 重大事件与重要节点应对
当已知即将发生或正在发生重大事件（如FOMC、非农、CPI、监管升级、网升/硬分叉等），按以下执行：
- 事件发生前30分钟：自动将网格模式转为“事件防御”。
- 降低杠杆至≤3倍，每格数量降至原30%，或直接pause_grid等待事件落地。
- 若15m出现极端波动（实体>ATR*3），立即紧急熔断（见风控）。
- 事件结束后至少等待1小时，待带宽回落、ADX正常再重新判断恢复。

## 资金费率处理
- 在持有多仓或空仓时，必须计入当前资金费率成本/收益。
- 若资金费率绝对值>0.1%，且网格成交后预计持仓超过一个结算周期（通常8小时），则必须将该网格数量调降50%，并在funding_rate_warning中详细说明。
- 计算利润率时，若扣除预估费率后实际利润<0.2%，输出警告并建议缩小持仓周期或暂停网格。

## 网格参数计算

### 上下界（基于1h）
1. 初始上/下界 = 价格 ± 2σ_price（近20根收盘价标准差）。
2. 上界 = max(初始上界, 近20最高, 布林上轨, 上方整数关口)
   下界 = min(初始下界, 近20最低, 布林下轨, 下方整数关口)
3. 宽度≥ATR*10，若不足等比扩展；若>ATR*30则内缩至ATR*20并保持价格居中。
4. 价格距边界必须>宽度1%，否则扩展。

### 高斯分布生成
- μ = (上界+下界)/2，σ = 宽度/4，分位数 p_i=(i-0.5)/N，price_i=μ+σ*Φ⁻¹(p_i)。
- 超出边界的price_i设为边界值；若连续多个价格截断至同一值，仅保留一个，后续通过额外插值补足至N。
- 方向：高于当前价→sell，否则→buy。列表升序。

### 利润率
- 取所有相邻(buy,sell)对的利润率，min值需>0.3%，否则减N或扩宽度重算。
- 考虑资金费率后实际利润率若<0.2%，输出警告。

### 每格资金
- 有效比例 = 生成网格中落在[μ-σ,μ+σ]的实际比例。
- 每格USDT = 总投资/(N*有效比例)，单格币数 = 每格USDT/(杠杆*price)。
- 总挂单（含杠杆）≤投资额×杠杆，超出则比例缩量。

## 网格调整对比
- 若新旧边界差<1%且N不变→hold，否则才adjust/pause+重建，对比仅看活跃挂单。

## 操作指令
- place_buy_limit: 在指定价格挂买单，参数包括价格、数量(币数或USDT)，并指定订单类型(限价单)。
- place_sell_limit: 在指定价格挂卖单，参数包括价格、数量(币数或USDT)，并指定订单类型(限价单)。
- cancel_order: 取消指定订单，参数为 order_id。
- cancel_all_orders: 取消所有挂单。
- pause_grid: 暂停网格策略，不再新挂单，但现有挂单如何处理？可能保留或取消？需要说明。
- resume_grid: 恢复网格，重新按照新的或原有的网格参数挂单。
- adjust_grid: 调整网格上下界并更新挂单，可能涉及取消部分订单和新增订单。
- reduce_position: 减小每格数量至当前的一半，可能是针对已有持仓或挂单数量的调整。
- hold: 不执行任何操作。

## 风控规则
1. 总已成交仓位≤总投资30%，超出减仓或暂停。
2. 贯穿网格浮亏≤总投资15%，否则取消所有单并暂停。
3. 价格连续2根1h收盘在界外→ cancel_all + pause。
4. 15m反向尖破放量→保护性暂停。
5. 连续3对完整交易（买-卖）净亏损→每格量减半，再犯则暂停。
6. 杠杆：震荡≤10x，高波动/事件≤3x，趋势暂停时0。
7. 紧急熔断：15m单根涨跌幅>ATR*3 → 立即取消所有挂单并暂停，待波动正常。

## 输出JSON（无代码块）
{
  "market_regime": "ranging|trending_up|trending_down|volatile|transition",
  "confidence": 0.0-1.0,
  "recommended_action": "place_buy_limit|place_sell_limit|cancel_order|cancel_all_orders|pause_grid|resume_grid|adjust_grid|reduce_position|hold",
  "action_reason": "含主周期+辅助信号依据(80字内)",
  "upper_price": 数字（网格上界）,
  "lower_price": 数字（网格下界）,
  "grid_count": 数字（网格层数）,
  "grid_profit_pct": 数字（每格利润率%）,
  "quantity_per_grid": 数字（每格数量，USDT）,
  "leverage": 数字（杠杆）,
  "funding_rate_warning": "资金费率风险说明(若有，否则填'none')",
  "event_impact": "事件影响说明(若无事件填'none')",
  "grid_levels":  [
    { "level": 1, "price": 数字, "side": "buy", "quantity_usdt": 数字 },
    { "level": 2, "price": 数字, "side": "buy", "quantity_usdt": 数字 },
    ...
    { "level": N, "price": 数字, "side": "sell", "quantity_usdt": 数字 }
  ],
  "analysis": "多周期信号、区间逻辑、风险(300字内)",
  "risk_warning": "主要风险提示(100字内)"
}"#
}

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
    funding_next_time: String,
    h1_atr_sma20: f64,
    h1_candle_body: f64,
    h1_bars_outside_band: i32,
    h1_bandwidth_5bars_ago: f64,
    h1_high_20: f64,
    h1_low_20: f64,
    nearest_round_up: f64,
    nearest_round_down: f64,
    m15_current_price: f64,
    m15_bb_width_pct: f64,
    m15_atr: f64,
    m15_atr_sma20: f64,
    m15_adx: f64,
    m15_bars_outside_band: i32,
    m15_ema20: f64,
    m15_ema50: f64,
    h4_ema20: f64,
    h4_ema50: f64,
    h4_adx: f64,
    h4_bb_width_pct: f64,
    // 账户余额
    total_balance: f64,
    available_balance: f64,
    used_margin: f64,
}

async fn compute_grid_indicators(
    klines_1h: &[Kline],
    klines_4h: &[Kline],
    klines_15m: &[Kline],
    exchange: &dyn crate::trading::exchange::Exchange,
    symbol: &str,
) -> GridIndicators {
    let last_idx = klines_1h.len().saturating_sub(1);
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

    let change_1h = if last_idx >= 1 && klines_1h[last_idx.saturating_sub(1)].close > 0.0 {
        (current_price - klines_1h[last_idx.saturating_sub(1)].close) / klines_1h[last_idx.saturating_sub(1)].close * 100.0
    } else { 0.0 };

    let change_4h = if last_idx >= 4 && klines_1h[last_idx.saturating_sub(4)].close > 0.0 {
        (current_price - klines_1h[last_idx.saturating_sub(4)].close) / klines_1h[last_idx.saturating_sub(4)].close * 100.0
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

    let macd = indicators::macd_at(klines_1h, last_idx, 12, 26);
    let macd_signal = indicators::macd_signal_at(klines_1h, last_idx, 12, 26, 9);
    let macd_histogram = indicators::macd_histogram_at(klines_1h, last_idx, 12, 26, 9);
    let adx = indicators::adx_at(klines_1h, last_idx, 14);

    let funding_result = exchange.get_funding_rate(symbol).await.ok();
    let funding_rate = funding_result.as_ref().map(|fr| fr.rate).unwrap_or(0.0);
    let funding_next_time = funding_result
        .as_ref()
        .and_then(|fr| fr.next_funding_time)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
        .unwrap_or_else(|| "N/A".to_string());

    // ── 1h 新增指标 ──
    let h1_atr_sma20 = if klines_1h.len() >= 20 {
        let atr_series = indicators::atr(klines_1h, 14);
        indicators::sma_at_from(&atr_series, last_idx, 20)
    } else { 0.0 };

    let last_kline = klines_1h.last();
    let h1_candle_body = last_kline.map(|k| k.close - k.open).unwrap_or(0.0);

    let h1_bars_outside_band = indicators::compute_bars_outside_band(klines_1h, bb_upper, bb_lower);

    let h1_bandwidth_5bars_ago = if last_idx >= 5 {
        indicators::bbands_width_at(klines_1h, last_idx.saturating_sub(5), 20, 2.0)
    } else { 0.0 };

    let h1_high_20 = indicators::highest_at(klines_1h, last_idx, 20);
    let h1_low_20 = indicators::lowest_at(klines_1h, last_idx, 20);

    let nearest_round_up = indicators::find_round_number(current_price, true);
    let nearest_round_down = indicators::find_round_number(current_price, false);

    // ── 4h 指标 ──
    let h4_last = klines_4h.len().saturating_sub(1);
    let h4_ema20 = if !klines_4h.is_empty() { indicators::ema_at(klines_4h, h4_last, 20) } else { 0.0 };
    let h4_ema50 = if klines_4h.len() >= 50 { indicators::ema_at(klines_4h, h4_last, 50) } else { 0.0 };
    let h4_adx = if !klines_4h.is_empty() { indicators::adx_at(klines_4h, h4_last, 14) } else { 0.0 };
    let h4_bb_width_pct = if !klines_4h.is_empty() { indicators::bbands_width_at(klines_4h, h4_last, 20, 2.0) } else { 0.0 };
    let ema_4h = h4_ema20;

    // ── 15m 指标 ──
    let m15_last = klines_15m.len().saturating_sub(1);
    let m15_current_price = klines_15m.last().map(|k| k.close).unwrap_or(0.0);
    let m15_bb_width_pct = if !klines_15m.is_empty() { indicators::bbands_width_at(klines_15m, m15_last, 20, 2.0) } else { 0.0 };
    let m15_atr = if !klines_15m.is_empty() { indicators::atr_at(klines_15m, m15_last, 14) } else { 0.0 };
    let m15_atr_sma20 = if klines_15m.len() >= 20 {
        let atr_series = indicators::atr(klines_15m, 14);
        indicators::sma_at_from(&atr_series, m15_last, 20)
    } else { 0.0 };
    let m15_adx = if !klines_15m.is_empty() { indicators::adx_at(klines_15m, m15_last, 14) } else { 0.0 };
    let (m15_bb_upper, _, m15_bb_lower) = if !klines_15m.is_empty() {
        indicators::bbands_at(klines_15m, m15_last, 20, 2.0)
    } else { (0.0, 0.0, 0.0) };
    let m15_bars_outside_band = indicators::compute_bars_outside_band(klines_15m, m15_bb_upper, m15_bb_lower);
    let m15_ema20 = if !klines_15m.is_empty() { indicators::ema_at(klines_15m, m15_last, 20) } else { 0.0 };
    let m15_ema50 = if klines_15m.len() >= 50 { indicators::ema_at(klines_15m, m15_last, 50) } else { 0.0 };

    // 获取账户余额
    let (total_balance, available_balance, used_margin) = exchange.get_balances().await
        .map(|bs| {
            let usdt = bs.iter().find(|b| b.asset.eq_ignore_ascii_case("USDT"));
            match usdt {
                Some(b) => (b.total, b.free, b.used),
                None => (0.0, 0.0, 0.0),
            }
        })
        .unwrap_or((0.0, 0.0, 0.0));

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
        funding_next_time,
        h1_atr_sma20,
        h1_candle_body,
        h1_bars_outside_band,
        h1_bandwidth_5bars_ago,
        h1_high_20,
        h1_low_20,
        nearest_round_up,
        nearest_round_down,
        m15_current_price,
        m15_bb_width_pct,
        m15_atr,
        m15_atr_sma20,
        m15_adx,
        m15_bars_outside_band,
        m15_ema20,
        m15_ema50,
        h4_ema20,
        h4_ema50,
        h4_adx,
        h4_bb_width_pct,
        total_balance,
        available_balance,
        used_margin,
    }
}

fn build_user_prompt(template: &str, ind: &GridIndicators, bot: &crate::models::GridBot) -> String {
    let ema_distance_pct = if ind.ema50 > 0.0 {
        (ind.ema20 - ind.ema50) / ind.ema50 * 100.0
    } else { 0.0 };

    let margin_usage_rate = if ind.total_balance > 0.0 {
        ind.used_margin / ind.total_balance * 100.0
    } else { 0.0 };
    let grid_status = match bot.status {
        crate::models::StrategyStatus::Running => "running",
        crate::models::StrategyStatus::Paused => "paused",
        _ => "empty",
    };
    let last_adjust_time = bot.last_adjusted_at
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let current_grid_config = if grid_status == "empty" {
        "none".to_string()
    } else {
        serde_json::json!({
            "upper_price": bot.upper_price,
            "lower_price": bot.lower_price,
            "grid_count": bot.grid_count,
            "grid_profit_pct": bot.grid_profit_pct,
            "quantity_per_grid": bot.quantity_per_grid,
        }).to_string()
    };

    let h1_atr_sma20_str = if ind.h1_atr_sma20.is_nan() { "N/A".to_string() } else { format!("{:.4}", ind.h1_atr_sma20) };
    let m15_atr_sma20_str = if ind.m15_atr_sma20.is_nan() { "N/A".to_string() } else { format!("{:.4}", ind.m15_atr_sma20) };

    template
        .replace("{timestamp}", &chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
        .replace("{symbol}", &bot.symbol)
        .replace("{total_balance}", &format!("{:.2}", ind.total_balance))
        .replace("{available_balance}", &format!("{:.2}", ind.available_balance))
        .replace("{used_margin}", &format!("{:.2}", ind.used_margin))
        .replace("{margin_usage_rate}", &format!("{:.1}", margin_usage_rate))
        .replace("{leverage}", &bot.leverage.to_string())
        .replace("{grid_status}", grid_status)
        .replace("{last_adjust_time}", &last_adjust_time)
        .replace("{consecutive_losses}", "0")
        .replace("{current_grid_config}", &current_grid_config)
        .replace("{position_base}", "0")
        .replace("{position_side}", "long")
        .replace("{entry_price}", "0")
        .replace("{unrealized_pnl}", &format!("{:.2}", bot.total_pnl))
        .replace("{open_orders}", "[]")
        .replace("{funding_rate}", &format!("{:.6}", ind.funding_rate))
        .replace("{funding_next_time}", &ind.funding_next_time)
        .replace("{event_flag}", "false")
        .replace("{event_description}", "")
        .replace("{h1_current_price}", &format!("{:.2}", ind.current_price))
        .replace("{h1_bb_upper}", &format!("{:.2}", ind.bb_upper))
        .replace("{h1_bb_middle}", &format!("{:.2}", ind.bb_middle))
        .replace("{h1_bb_lower}", &format!("{:.2}", ind.bb_lower))
        .replace("{h1_bb_width_pct}", &format!("{:.2}", ind.bb_width))
        .replace("{h1_ema20}", &format!("{:.2}", ind.ema20))
        .replace("{h1_ema50}", &format!("{:.2}", ind.ema50))
        .replace("{h1_ema_distance_pct}", &format!("{:+.2}", ema_distance_pct))
        .replace("{h1_adx}", &format!("{:.2}", ind.adx))
        .replace("{h1_atr}", &format!("{:.4}", ind.atr))
        .replace("{h1_atr_sma20}", &h1_atr_sma20_str)
        .replace("{h1_candle_body}", &format!("{:+.4}", ind.h1_candle_body))
        .replace("{h1_bars_outside_band}", &ind.h1_bars_outside_band.to_string())
        .replace("{h1_bandwidth_5bars_ago}", &format!("{:.2}", ind.h1_bandwidth_5bars_ago))
        .replace("{h1_high_20}", &format!("{:.2}", ind.h1_high_20))
        .replace("{h1_low_20}", &format!("{:.2}", ind.h1_low_20))
        .replace("{nearest_round_up}", &format!("{:.2}", ind.nearest_round_up))
        .replace("{nearest_round_down}", &format!("{:.2}", ind.nearest_round_down))
        .replace("{m15_current_price}", &format!("{:.2}", ind.m15_current_price))
        .replace("{m15_bb_width_pct}", &format!("{:.2}", ind.m15_bb_width_pct))
        .replace("{m15_atr}", &format!("{:.4}", ind.m15_atr))
        .replace("{m15_atr_sma20}", &m15_atr_sma20_str)
        .replace("{m15_adx}", &format!("{:.2}", ind.m15_adx))
        .replace("{m15_bars_outside_band}", &ind.m15_bars_outside_band.to_string())
        .replace("{m15_ema20}", &format!("{:.2}", ind.m15_ema20))
        .replace("{m15_ema50}", &format!("{:.2}", ind.m15_ema50))
        .replace("{h4_ema20}", &format!("{:.2}", ind.h4_ema20))
        .replace("{h4_ema50}", &format!("{:.2}", ind.h4_ema50))
        .replace("{h4_adx}", &format!("{:.2}", ind.h4_adx))
        .replace("{h4_bb_width_pct}", &format!("{:.2}", ind.h4_bb_width_pct))
        .replace("{trigger_reason}", "manual")
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
) -> Result<(Vec<Kline>, Vec<Kline>, Vec<Kline>), (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let exchange_key = super::market::ensure_exchange(state, exchange_name, MarketType::Perpetual).await?;
    let exchange = state.exchange_registry.get(&exchange_key).unwrap();

    let now_ms = chrono::Utc::now().timestamp_millis();
    let start_1h = now_ms - 200 * 3600 * 1000;
    let start_4h = now_ms - 50 * 4 * 3600 * 1000;
    let start_15m = now_ms - 200 * 15 * 60 * 1000;

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

    let klines_15m = match exchange.get_klines_range(symbol, "15m", start_15m, now_ms).await {
        Ok(k) => k,
        Err(e) => {
            tracing::warn!("Failed to fetch 15m klines for {}: {}", symbol, e);
            Vec::new()
        }
    };

    Ok((klines_1h, klines_4h, klines_15m))
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

    let (klines_1h, klines_4h, klines_15m) = fetch_klines(&state, exchange_name, symbol).await?;
    let exchange_key = super::market::ensure_exchange(&state, exchange_name, MarketType::Perpetual).await?;
    let exchange = state.exchange_registry.get(&exchange_key).unwrap();
    let ind = compute_grid_indicators(&klines_1h, &klines_4h, &klines_15m, exchange.as_ref(), symbol).await;

    let system_prompt = match body.system_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => bot.system_prompt.as_deref().unwrap_or_else(|| default_grid_system_prompt()).to_owned(),
    };

    let user_prompt_template = match body.user_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => bot.user_prompt.as_deref().unwrap_or(DEFAULT_USER_PROMPT_TEMPLATE).to_owned(),
    };
    let user_prompt = build_user_prompt(&user_prompt_template, &ind, &bot);

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
            market_regime, ai_analysis, grid_levels_json, system_prompt, user_prompt,
            dynamic_adjust, adjust_interval_secs
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14::jsonb, $15, $16, $17, $18)
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
    .bind(&None::<serde_json::Value>)
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

    let level_quantities: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
        r#"SELECT grid_level, SUM(quantity) FROM qd_grid_trades WHERE bot_id = $1 GROUP BY grid_level"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let buy_fill_prices: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
        r#"SELECT grid_level, AVG(price) FROM qd_grid_trades WHERE bot_id = $1 AND side = 'buy' GROUP BY grid_level"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let sell_fill_prices: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
        r#"SELECT grid_level, MAX(price) FROM qd_grid_trades WHERE bot_id = $1 AND side = 'sell' GROUP BY grid_level"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let buy_quantities: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
        r#"SELECT grid_level, SUM(quantity) FROM qd_grid_trades WHERE bot_id = $1 AND side = 'buy' GROUP BY grid_level"#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let sell_quantities: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
        r#"SELECT grid_level, SUM(quantity) FROM qd_grid_trades WHERE bot_id = $1 AND side = 'sell' GROUP BY grid_level"#,
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
    let profit_factor = 1.0 + bot.grid_profit_pct / 100.0;
    let mid_price = (bot.upper_price + bot.lower_price) / 2.0;

    let llm_levels: Vec<serde_json::Value> = bot.grid_levels_json
        .as_ref()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();

    let mut grid_levels = Vec::new();
    for i in 0..=bot.grid_count {
        let price = bot.lower_price + grid_spacing * i as f64;
        let llm_level = llm_levels.iter().find(|v| v["level"].as_i64() == Some(i as i64));
        let side = if let Some(l) = llm_level {
            l["side"].as_str().unwrap_or("buy")
        } else {
            if price < mid_price { "buy" } else { "sell" }
        };
        let (buy_price, sell_price) = if side == "buy" {
            (price, price * profit_factor)
        } else {
            (price / profit_factor, price)
        };
        let quantity = level_quantities.get(&i).copied().unwrap_or(0.0);
        let buy_qty = buy_quantities.get(&i).copied().unwrap_or(0.0);
        let sell_qty = sell_quantities.get(&i).copied().unwrap_or(0.0);
        let hold_quantity = if side == "buy" { (buy_qty - sell_qty).max(0.0) } else { (sell_qty - buy_qty).max(0.0) * -1.0 };
        let avg_buy_price = buy_fill_prices.get(&i).copied().unwrap_or(0.0);
        let last_sell_price = sell_fill_prices.get(&i).copied().unwrap_or(0.0);
        let has_buy = buy_fill_prices.contains_key(&i);
        let has_sell = sell_fill_prices.contains_key(&i);
        grid_levels.push(serde_json::json!({
            "level": i,
            "price": price,
            "side": side,
            "buy_price": buy_price,
            "sell_price": sell_price,
            "open_price": if side == "buy" { buy_price } else { sell_price },
            "close_price": if side == "buy" { sell_price } else { buy_price },
            "filled": filled_set.contains(&i),
            "quantity": quantity,
            "buy_filled": has_buy,
            "sell_filled": has_sell,
            "hold_quantity": hold_quantity,
            "avg_buy_price": avg_buy_price,
            "last_fill_price": if has_sell { last_sell_price } else if has_buy { avg_buy_price } else { 0.0 },
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
        let (klines_1h, klines_4h, klines_15m) = fetch_klines(&state, &bot.exchange, &bot.symbol).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::err(format!("Failed to fetch klines: {}", e.1 .0.error.unwrap_or_default())))))?;
        let exchange_key = super::market::ensure_exchange(&state, &bot.exchange, MarketType::Perpetual).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::err(format!("Failed to init exchange: {}", e.1 .0.error.unwrap_or_default())))))?;
        let exchange = state.exchange_registry.get(&exchange_key).unwrap();
        let ind = compute_grid_indicators(&klines_1h, &klines_4h, &klines_15m, exchange.as_ref(), &bot.symbol).await;

        let system_prompt = bot.system_prompt.as_deref().unwrap_or_else(|| default_grid_system_prompt()).to_owned();
        let user_prompt_template = bot.user_prompt.as_deref().unwrap_or(DEFAULT_USER_PROMPT_TEMPLATE).to_owned();
        let user_prompt = build_user_prompt(&user_prompt_template, &ind, &bot);

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
                let new_grid_levels_json = ai.result.get("grid_levels")
                    .filter(|v| v.is_array())
                    .cloned();

                // 更新 bot 参数
                let updated = sqlx::query_as::<_, GridBot>(
                    r#"UPDATE qd_grid_bots SET
                        upper_price = $1, lower_price = $2, grid_count = $3,
                        grid_profit_pct = $4, quantity_per_grid = $5, leverage = $6,
                        market_regime = $7, ai_analysis = $8, grid_levels_json = $9::jsonb,
                        system_prompt = $10, user_prompt = $11,
                        updated_at = NOW()
                       WHERE id = $12 RETURNING *"#,
                )
                .bind(new_upper_price)
                .bind(new_lower_price)
                .bind(new_grid_count)
                .bind(new_grid_profit_pct)
                .bind(new_quantity_per_grid)
                .bind(new_leverage)
                .bind(&new_market_regime)
                .bind(&new_analysis)
                .bind(&new_grid_levels_json)
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

#[derive(Debug, Deserialize)]
pub struct DeleteBotParams {
    pub close_position: Option<bool>,
}

pub async fn delete_bot(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(params): Query<DeleteBotParams>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let user_id = parse_user_id(&auth)?;
    let close_position = params.close_position.unwrap_or(false);

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
            if b.status == StrategyStatus::Running && !close_position {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<serde_json::Value>::err(
                        "Cannot delete a running bot without close_position. Use close_position=true to stop and close positions.",
                    )),
                ));
            }

            if let Some(ref grid_cmd_tx) = state.grid_cmd_tx {
                if let Err(e) = grid_cmd_tx.send(crate::bot::semi_automatic_grid::types::GridCommand::DeleteBot {
                    bot_id: id,
                    close_position,
                }).await {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiResponse::<serde_json::Value>::err(format!(
                            "Failed to send delete command: {}", e
                        ))),
                    ));
                }
            } else {
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
            }
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<serde_json::Value>::err("Grid bot not found")),
            ));
        }
    }

    Ok(Json(ApiResponse::ok_with_message(
        serde_json::json!({ "deleted": true, "close_position": close_position }),
        if close_position {
            "Grid bot deleted with positions closed"
        } else {
            "Grid bot deleted (positions remain open)"
        },
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

    let bot = sqlx::query_as::<_, GridBot>(
        r#"SELECT * FROM qd_grid_bots WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .ok()
    .flatten();

    let grid_levels = if let Some(ref b) = bot {
        let filled_levels: Vec<i32> = sqlx::query_scalar(
            r#"SELECT DISTINCT grid_level FROM qd_grid_trades WHERE bot_id = $1"#,
        )
        .bind(id)
        .fetch_all(&state.db_pool)
        .await
        .unwrap_or_default();

        let filled_set: std::collections::HashSet<i32> = filled_levels.into_iter().collect();

        let level_quantities: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
            r#"SELECT grid_level, SUM(quantity) FROM qd_grid_trades WHERE bot_id = $1 GROUP BY grid_level"#,
        )
        .bind(id)
        .fetch_all(&state.db_pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        let buy_fill_prices: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
            r#"SELECT grid_level, AVG(price) FROM qd_grid_trades WHERE bot_id = $1 AND side = 'buy' GROUP BY grid_level"#,
        )
        .bind(id)
        .fetch_all(&state.db_pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        let sell_fill_prices: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
            r#"SELECT grid_level, MAX(price) FROM qd_grid_trades WHERE bot_id = $1 AND side = 'sell' GROUP BY grid_level"#,
        )
        .bind(id)
        .fetch_all(&state.db_pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        let buy_quantities: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
            r#"SELECT grid_level, SUM(quantity) FROM qd_grid_trades WHERE bot_id = $1 AND side = 'buy' GROUP BY grid_level"#,
        )
        .bind(id)
        .fetch_all(&state.db_pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        let sell_quantities: std::collections::HashMap<i32, f64> = sqlx::query_as::<_, (i32, f64)>(
            r#"SELECT grid_level, SUM(quantity) FROM qd_grid_trades WHERE bot_id = $1 AND side = 'sell' GROUP BY grid_level"#,
        )
        .bind(id)
        .fetch_all(&state.db_pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        let grid_spacing = if b.grid_count > 1 {
            (b.upper_price - b.lower_price) / b.grid_count as f64
        } else {
            0.0
        };
        let profit_factor = 1.0 + b.grid_profit_pct / 100.0;
        let mid_price = (b.upper_price + b.lower_price) / 2.0;

        let llm_levels: Vec<serde_json::Value> = b.grid_levels_json
            .as_ref()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();

        (0..=b.grid_count)
            .map(|i| {
                let price = b.lower_price + grid_spacing * i as f64;
                let llm_level = llm_levels.iter().find(|v| v["level"].as_i64() == Some(i as i64));
                let side = if let Some(l) = llm_level {
                    l["side"].as_str().unwrap_or("buy")
                } else {
                    if price < mid_price { "buy" } else { "sell" }
                };
                let (buy_price, sell_price) = if side == "buy" {
                    (price, price * profit_factor)
                } else {
                    (price / profit_factor, price)
                };
                let quantity = level_quantities.get(&i).copied().unwrap_or(0.0);
                let buy_qty = buy_quantities.get(&i).copied().unwrap_or(0.0);
                let sell_qty = sell_quantities.get(&i).copied().unwrap_or(0.0);
                let hold_quantity = if side == "buy" { (buy_qty - sell_qty).max(0.0) } else { (sell_qty - buy_qty).max(0.0) * -1.0 };
                let avg_buy_price = buy_fill_prices.get(&i).copied().unwrap_or(0.0);
                let last_sell_price = sell_fill_prices.get(&i).copied().unwrap_or(0.0);
                let has_buy = buy_fill_prices.contains_key(&i);
                let has_sell = sell_fill_prices.contains_key(&i);
                serde_json::json!({
                    "level": i,
                    "price": price,
                    "side": side,
                    "buy_price": buy_price,
                    "sell_price": sell_price,
                    "open_price": if side == "buy" { buy_price } else { sell_price },
                    "close_price": if side == "buy" { sell_price } else { buy_price },
                    "filled": filled_set.contains(&i),
                    "quantity": quantity,
                    "buy_filled": has_buy,
                    "sell_filled": has_sell,
                    "hold_quantity": hold_quantity,
                    "avg_buy_price": avg_buy_price,
                    "last_fill_price": if has_sell { last_sell_price } else if has_buy { avg_buy_price } else { 0.0 },
                })
            })
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    let total_pages = (total.0 + page_size - 1) / page_size;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "items": trades,
        "total": total.0,
        "page": page,
        "page_size": page_size,
        "total_pages": total_pages,
        "grid_levels": grid_levels,
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

    let (klines_1h, klines_4h, klines_15m) = fetch_klines(&state, &bot.exchange, &bot.symbol).await?;
    let exchange_key = super::market::ensure_exchange(&state, &bot.exchange, MarketType::Perpetual).await?;
    let exchange = state.exchange_registry.get(&exchange_key).unwrap();
    let ind = compute_grid_indicators(&klines_1h, &klines_4h, &klines_15m, exchange.as_ref(), &bot.symbol).await;

    let system_prompt = match body.system_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => bot.system_prompt.as_deref().unwrap_or_else(|| default_grid_system_prompt()).to_owned(),
    };

    let user_prompt_template = match body.user_prompt.as_deref() {
        Some(s) => s.to_owned(),
        None => bot.user_prompt.as_deref().unwrap_or(DEFAULT_USER_PROMPT_TEMPLATE).to_owned(),
    };
    let user_prompt = build_user_prompt(&user_prompt_template, &ind, &bot);

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
            let new_grid_levels_json = ai.result.get("grid_levels")
                .filter(|v| v.is_array())
                .cloned();

            let updated = sqlx::query_as::<_, GridBot>(
                r#"UPDATE qd_grid_bots SET
                    upper_price = $1, lower_price = $2, grid_count = $3,
                    grid_profit_pct = $4, quantity_per_grid = $5, leverage = $6,
                    market_regime = $7, ai_analysis = $8, grid_levels_json = $9::jsonb,
                    last_adjusted_at = NOW(),
                    updated_at = NOW()
                   WHERE id = $10 RETURNING *"#,
            )
            .bind(new_upper_price)
            .bind(new_lower_price)
            .bind(new_grid_count)
            .bind(new_grid_profit_pct)
            .bind(new_quantity_per_grid)
            .bind(new_leverage)
            .bind(&new_market_regime)
            .bind(&new_analysis)
            .bind(&new_grid_levels_json)
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
