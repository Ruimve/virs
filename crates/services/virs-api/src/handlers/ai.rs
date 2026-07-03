//! AI analysis handlers.

use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use virs_error::{VirsError, VirsResult};

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

/// Resolve the base URL for a known LLM provider.
pub fn resolve_provider_base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "deepseek" => Some("https://api.deepseek.com"),
        "openai" => Some("https://api.openai.com/v1"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        _ => None,
    }
}

/// Resolve the default model for a known LLM provider.
pub fn resolve_provider_model(provider: &str) -> Option<&'static str> {
    match provider {
        "deepseek" => Some("deepseek-chat"),
        "openai" => Some("gpt-4o"),
        "openrouter" => Some("deepseek/deepseek-chat"),
        _ => None,
    }
}

pub async fn ai_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers)?;

    // Check if any AI credentials are configured
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
    let _user_id = extract_user_id(&headers)?;

    let symbol = body["symbol"].as_str().unwrap_or("");
    let exchange = body["exchange"].as_str().unwrap_or("");

    if symbol.is_empty() {
        return Err(VirsError::bad_request("symbol is required"));
    }

    // Fetch market data for context
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

    match call_llm_with_fallback(&state, system_prompt, &user_prompt).await {
        Ok(result) => Ok(Json(ApiResponse::ok(result))),
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
    let _user_id = extract_user_id(&headers)?;

    let symbol = body["symbol"].as_str().unwrap_or("");
    let question = body["question"].as_str().unwrap_or("");

    if symbol.is_empty() || question.is_empty() {
        return Err(VirsError::bad_request("symbol and question are required"));
    }

    let system_prompt = r#"You are a trading education assistant. Explain trading concepts and market conditions clearly.
Respond in JSON format with:
{
  "explanation": "...",
  "key_points": [...],
  "risk_warning": "..."
}"#;

    let user_prompt = format!("Symbol: {}\nQuestion: {}", symbol, question,);

    match call_llm_with_fallback(&state, system_prompt, &user_prompt).await {
        Ok(result) => Ok(Json(ApiResponse::ok(result))),
        Err(e) => Err(VirsError::bad_request(format!("AI explain failed: {}", e))),
    }
}

pub async fn recommend_strategy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, VirsError> {
    let _user_id = extract_user_id(&headers)?;

    let symbol = body["symbol"].as_str().unwrap_or("");
    let exchange = body["exchange"].as_str().unwrap_or("");
    let risk_tolerance = body["risk_tolerance"].as_str().unwrap_or_else(|| {
        tracing::warn!("risk_tolerance not provided — defaulting to 'medium'");
        "medium"
    });

    if symbol.is_empty() {
        return Err(VirsError::bad_request("symbol is required"));
    }

    let current_price = fetch_price_from_kline(&state, exchange, symbol).await?;

    let system_prompt = r#"You are a trading strategy advisor. Recommend a trading strategy based on market conditions.
Respond in JSON format with:
{
  "strategy": "grid|auto|manual",
  "strategy_details": {
    "grid_count": 10,
    "upper_price": 0,
    "lower_price": 0,
    "grid_profit_pct": 0.5,
    "leverage": 5
  },
  "reasoning": "...",
  "risk_warning": "..."
}"#;

    let user_prompt = format!(
        "Symbol: {}, Exchange: {}, Current Price: {:.2}, Risk Tolerance: {}\nPlease recommend a trading strategy.",
        symbol, exchange, current_price, risk_tolerance,
    );

    match call_llm_with_fallback(&state, system_prompt, &user_prompt).await {
        Ok(result) => Ok(Json(ApiResponse::ok(result))),
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
        .get_klines_async(exchange, symbol, virs_market::Timeframe::M1)
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

async fn call_llm_with_fallback(
    state: &AppState,
    system_prompt: &str,
    user_prompt: &str,
) -> VirsResult<serde_json::Value> {
    // Try to find AI credentials from database
    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key
           FROM qd_ai_credentials ORDER BY created_at DESC LIMIT 1"#,
    )
    .fetch_optional(&state.db_pool)
    .await?;

    let (api_key, base_url, model) = match row {
        Some((provider, encrypted_key)) => {
            let decrypted_key = virs_utils::crypto::decrypt_with_key(&encrypted_key, &state.encryption_key)?;

            let resolved_base_url = match resolve_provider_base_url(&provider) {
                Some(url) => url.to_string(),
                None => format!("https://api.{}.com", provider),
            };

            let resolved_model = resolve_provider_model(&provider)
                .ok_or_else(|| {
                    VirsError::config(format!(
                        "Unknown AI provider '{}' — cannot resolve default model. \
                         Supported providers: deepseek, openai, openrouter",
                        provider
                    ))
                })?
                .to_string();

            (decrypted_key, resolved_base_url, resolved_model)
        }
        None => {
            return Err(VirsError::unauthorized(
                "No AI API key configured. Set AI credentials via the wizard.",
            ));
        }
    };

    let http_client = &state.http_client;
    let result = virs_bot::common::ai_client::call_llm_api(
        http_client,
        &api_key,
        &base_url,
        &model,
        system_prompt,
        user_prompt,
        "virs-api",
    )
    .await?;

    Ok(result.content)
}
