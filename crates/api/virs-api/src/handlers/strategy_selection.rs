

use virs_error::VirsError;
use virs_indicator::IndicatorSpec;
use virs_prompt::PromptLoader;
use virs_type::{StrategyType, Timeframe};

use crate::state::AppState;


pub async fn select_strategy_by_llm(
    state: &AppState,
    loader: &PromptLoader,
    strategies: &[String],
    exchange: &str,
    symbol: &str,
    strategy_type: StrategyType,
) -> Result<String, VirsError> {

    let candles = state
        .kline_engine
        .get_klines(exchange, symbol, virs_type::Timeframe::H1)
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

    let klines: Vec<virs_type::Kline> = candles
        .iter()
        .map(|c| virs_type::Kline {
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


    let specs = [
        IndicatorSpec::CurrentPrice { tf: Timeframe::H1 },
        IndicatorSpec::Atr { tf: Timeframe::H1, period: 14 },
        IndicatorSpec::Rsi { tf: Timeframe::H1, period: 14 },
    ];
    let indicator_set = virs_indicator::compute_indicators(
        &klines, &[], &[], 0.0, "", Some(&specs),
    )?;
    let current_price = indicator_set
        .get_num(&IndicatorSpec::CurrentPrice { tf: Timeframe::H1 })
        .ok_or_else(|| VirsError::bad_request(format!(
            "No current price available for {} on {} — cannot select strategy",
            symbol, exchange
        )))?;
    let atr_val = indicator_set
        .get_num(&IndicatorSpec::Atr { tf: Timeframe::H1, period: 14 })
        .ok_or_else(|| VirsError::bad_request(format!(
            "No ATR data available for {} on {} — cannot select strategy",
            symbol, exchange
        )))?;
    let rsi_val = indicator_set
        .get_num(&IndicatorSpec::Rsi { tf: Timeframe::H1, period: 14 })
        .ok_or_else(|| VirsError::bad_request(format!(
            "No RSI data available for {} on {} — cannot select strategy",
            symbol, exchange
        )))?;


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
        .await
        .map_err(|e| VirsError::bad_request(format!("LLM strategy selection failed: {e}")))?;


    let parsed = &result.content;

    let selected = parsed["strategy_name"]
        .as_str()
        .ok_or_else(|| VirsError::bad_request("LLM did not return strategy_name"))?;


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
