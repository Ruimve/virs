//! LLM 策略选择共享逻辑。
//!
//! 被 auto_trade 和 grid handler 复用,避免两份近乎相同的 `select_strategy_by_llm`。

use virs_error::VirsError;
use virs_strategy::indicator::library::{atr, closes, rsi_at};
use virs_strategy::prompt::{PromptLoader, StrategyType};

use crate::state::AppState;

/// LLM 选择策略:获取市场快照 + 构造选择 prompt + 调用 LLM + 解析返回。
///
/// `strategy_type` 区分 Auto / Grid,决定从 PromptLoader 取哪类策略模板的元数据。
pub async fn select_strategy_by_llm(
    state: &AppState,
    loader: &PromptLoader,
    strategies: &[String],
    exchange: &str,
    symbol: &str,
    strategy_type: StrategyType,
) -> Result<String, VirsError> {
    // 获取 H1 K 线数据
    let candles = state
        .kline_engine
        .get_klines_async(exchange, symbol, virs_market::Timeframe::H1)
        .await
        .ok_or_else(|| {
            VirsError::bad_request(format!(
                "No kline data available for {} on {} — cannot select strategy",
                symbol, exchange
            ))
        })?;

    if candles.is_empty() {
        return Err(VirsError::bad_request(format!(
            "Kline data for {} on {} is empty — cannot select strategy",
            symbol, exchange
        )));
    }

    let klines: Vec<virs_models::Kline> = candles
        .iter()
        .map(|c| virs_models::Kline {
            open_time: c.open_time,
            open: c.open,
            high: c.high,
            low: c.low,
            close: c.close,
            volume: c.volume,
            close_time: c.close_time,
            quote_volume: c.quote_volume,
            trades: c.trades,
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            interval: "1h".to_string(),
        })
        .collect();

    // 计算基础指标
    let close_prices = closes(&klines);
    let current_price = close_prices.last().copied().unwrap_or(0.0);
    let atr_series = atr(&klines, 14);
    let atr_val = atr_series.last().copied().unwrap_or(0.0);
    let rsi_val = rsi_at(&klines, klines.len().saturating_sub(1), 14);

    // 获取策略元数据
    let mut strategy_details: Vec<serde_json::Value> = Vec::new();
    for name in strategies {
        if let Some(tpl) = loader.get(strategy_type, name).await {
            strategy_details.push(serde_json::json!({
                "name": tpl.name,
                "description": tpl.description,
            }));
        }
    }

    let system_prompt = r#"You are a trading strategy selector. Based on the current market conditions and available strategies, select the most suitable strategy.
Respond in JSON format with:
{
  "strategy_name": "the_strategy_name",
  "reason": "brief explanation",
  "confidence": 0.8
}"#;

    let user_prompt = format!(
        "Symbol: {}, Exchange: {}, Current Price: {:.2}\n\
         Market Indicators: ATR(14)={:.4}, RSI(14)={:.2}\n\
         Available Strategies: {}\n\
         Please select the best strategy for current market conditions.",
        symbol,
        exchange,
        current_price,
        atr_val,
        rsi_val,
        serde_json::to_string(&strategy_details).unwrap_or_default(),
    );

    let result = state
        .call_llm(system_prompt, &user_prompt, "strategy-selection")
        .await?;

    // 解析 LLM 返回的策略名（content 已是 JSON Value）
    let parsed = &result.content;

    let selected = parsed["strategy_name"]
        .as_str()
        .ok_or_else(|| VirsError::bad_request("LLM did not return strategy_name"))?;

    // 校验 LLM 返回的策略名在列表中
    if !strategies.iter().any(|s| s == selected) {
        return Err(VirsError::bad_request(format!(
            "LLM selected strategy '{selected}' which is not in the available list: {:?}",
            strategies
        )));
    }

    tracing::info!(
        selected = %selected,
        confidence = ?parsed["confidence"],
        "LLM strategy selection completed"
    );

    Ok(selected.to_string())
}
