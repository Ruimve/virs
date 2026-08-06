use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use virs_error::VirsError;
use virs_type::LlmProviderConfig;

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;


pub fn resolve_provider_base_url(provider: &str) -> Option<&'static str> {
    LlmProviderConfig::for_provider(provider).map(|c| c.base_url)
}

pub fn resolve_provider_model(provider: &str) -> Option<&'static str> {
    LlmProviderConfig::for_provider(provider).map(|c| c.default_model)
}

pub async fn ai_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;


    let rows: Vec<String> =
        sqlx::query_scalar(r#"SELECT DISTINCT provider FROM qd_ai_credentials"#)
            .fetch_all(&state.db_pool)
            .await?;

    let configured = !rows.is_empty();

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "configured": configured,
        "providers": rows,
    }))))
}

pub async fn optimize(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let symbol = body["symbol"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("symbol is required"))?;
    let exchange = body["exchange"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("exchange is required"))?;

    let current_price = fetch_price_from_kline(&state, exchange, symbol).await?;

    let system_prompt = r#"You are a trading strategy optimizer. Analyze the given market data and provide optimization suggestions.
Respond in JSON format with:
{
  "suggestions": [...],
  "risk_level": "low|medium|high",
  "recommended_strategy": "...",
  "reasoning": "..."
}"#;

    let user_prompt = format!(
        "Symbol: {}, Exchange: {}, Current Price: {:.2}\nPlease provide optimization suggestions for this trading pair.",
        symbol, exchange, current_price,
    );

    match state.call_llm(system_prompt, &user_prompt, "virs-api").await {
        Ok(result) => Ok(Json(ApiResponse::ok(result.content))),
        Err(e) => Err(VirsError::bad_request(format!(
            "AI optimization failed: {}",
            e
        ))),
    }
}

pub async fn explain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let symbol = body["symbol"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("symbol is required"))?;
    let question = body["question"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("question is required"))?;

    let system_prompt = r#"You are a trading education assistant. Explain trading concepts and market conditions clearly.
Respond in JSON format with:
{
  "explanation": "...",
  "key_points": [...],
  "risk_warning": "..."
}"#;

    let user_prompt = format!("Symbol: {}\nQuestion: {}", symbol, question,);

    match state.call_llm(system_prompt, &user_prompt, "virs-api").await {
        Ok(result) => Ok(Json(ApiResponse::ok(result.content))),
        Err(e) => Err(VirsError::bad_request(format!("AI explain failed: {}", e))),
    }
}

pub async fn recommend_strategy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers, &state.jwt_secret)?;

    let symbol = body["symbol"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("symbol is required"))?;
    let exchange = body["exchange"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| VirsError::bad_request("exchange is required"))?;
    let risk_tolerance = body["risk_tolerance"].as_str().ok_or_else(|| {
        VirsError::bad_request("risk_tolerance is required")
    })?;

    let current_price = fetch_price_from_kline(&state, exchange, symbol).await?;

    let system_prompt = r#"You are a trading strategy advisor. Recommend a trading strategy based on market conditions.
Respond in JSON format with:
{
  "strategy": "auto|manual",
  "strategy_details": {
    "leverage": 5
  },
  "reasoning": "...",
  "risk_warning": "..."
}"#;

    let user_prompt = format!(
        "Symbol: {}, Exchange: {}, Current Price: {:.2}, Risk Tolerance: {}\nPlease recommend a trading strategy.",
        symbol, exchange, current_price, risk_tolerance,
    );

    match state.call_llm(system_prompt, &user_prompt, "virs-api").await {
        Ok(result) => Ok(Json(ApiResponse::ok(result.content))),
        Err(e) => Err(VirsError::bad_request(format!(
            "AI recommend failed: {}",
            e
        ))),
    }
}

async fn fetch_price_from_kline(
    state: &AppState,
    exchange: &str,
    symbol: &str,
) -> Result<f64, VirsError> {
    let candles = state
        .kline_engine
        .get_klines_async(exchange, symbol, virs_type::Timeframe::M1)
        .await
        .ok_or_else(|| {
            VirsError::not_found(format!(
                "No kline data available for {} on {} — cannot determine current price",
                symbol, exchange
            ))
        })?;

    let last = candles.last().ok_or_else(|| {
        VirsError::not_found(format!(
            "Kline data empty for {} on {} — cannot determine current price",
            symbol, exchange
        ))
    })?;

    if last.close <= 0.0 {
        return Err(VirsError::config(format!(
            "Last kline close price is {} for {} on {} — invalid price data",
            last.close, symbol, exchange
        )));
    }

    Ok(last.close)
}
