//! AI analysis handlers.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::handlers::response::{extract_user_id, ApiResponse};
use crate::state::AppState;

pub async fn ai_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let _user_id = extract_user_id(&headers)?;

    // Check if any AI credentials are configured
    let rows: Vec<String> =
        sqlx::query_scalar(r#"SELECT DISTINCT provider FROM qd_ai_credentials"#)
            .fetch_all(&state.db_pool)
            .await
            .unwrap_or_default();

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
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let _user_id = extract_user_id(&headers)?;

    let symbol = body["symbol"].as_str().unwrap_or("");
    let exchange = body["exchange"].as_str().unwrap_or("");

    if symbol.is_empty() {
        return Ok(Json(ApiResponse::err("symbol is required")));
    }

    // Fetch market data for context
    let current_price = fetch_price_from_kline(&state, exchange, symbol).await;

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

    match call_llm_with_fallback(&state, &system_prompt, &user_prompt).await {
        Ok(result) => Ok(Json(ApiResponse::ok(result))),
        Err(e) => Ok(Json(ApiResponse::err(format!(
            "AI optimization failed: {}",
            e
        )))),
    }
}

pub async fn explain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let _user_id = extract_user_id(&headers)?;

    let symbol = body["symbol"].as_str().unwrap_or("");
    let question = body["question"].as_str().unwrap_or("");

    if symbol.is_empty() || question.is_empty() {
        return Ok(Json(ApiResponse::err("symbol and question are required")));
    }

    let system_prompt = r#"You are a trading education assistant. Explain trading concepts and market conditions clearly.
Respond in JSON format with:
{
  "explanation": "...",
  "key_points": [...],
  "risk_warning": "..."
}"#;

    let user_prompt = format!("Symbol: {}\nQuestion: {}", symbol, question,);

    match call_llm_with_fallback(&state, &system_prompt, &user_prompt).await {
        Ok(result) => Ok(Json(ApiResponse::ok(result))),
        Err(e) => Ok(Json(ApiResponse::err(format!("AI explain failed: {}", e)))),
    }
}

pub async fn recommend_strategy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    let _user_id = extract_user_id(&headers)?;

    let symbol = body["symbol"].as_str().unwrap_or("");
    let exchange = body["exchange"].as_str().unwrap_or("");
    let risk_tolerance = body["risk_tolerance"].as_str().unwrap_or("medium");

    if symbol.is_empty() {
        return Ok(Json(ApiResponse::err("symbol is required")));
    }

    let current_price = fetch_price_from_kline(&state, exchange, symbol).await;

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

    match call_llm_with_fallback(&state, &system_prompt, &user_prompt).await {
        Ok(result) => Ok(Json(ApiResponse::ok(result))),
        Err(e) => Ok(Json(ApiResponse::err(format!(
            "AI recommend failed: {}",
            e
        )))),
    }
}

async fn fetch_price_from_kline(state: &AppState, exchange: &str, symbol: &str) -> f64 {
    if let Some(candles) = state
        .kline_engine
        .get_klines_async(exchange, symbol, virs_market::Timeframe::M1)
        .await
    {
        if let Some(last) = candles.last() {
            if last.close > 0.0 {
                return last.close;
            }
        }
    }
    0.0
}

async fn call_llm_with_fallback(
    state: &AppState,
    system_prompt: &str,
    user_prompt: &str,
) -> anyhow::Result<serde_json::Value> {
    // Try to find AI credentials from database
    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT provider, encrypted_api_key
           FROM qd_ai_credentials ORDER BY created_at DESC LIMIT 1"#,
    )
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error: {}", e))?;

    let (api_key, base_url, model) = match row {
        Some((provider, encrypted_key)) => {
            let derived_key = virs_utils::crypto::derive_key(&state.encryption_key);
            let decrypted_key = virs_utils::crypto::decrypt(&encrypted_key, &derived_key)
                .map_err(|e| anyhow::anyhow!("Decryption error: {}", e))?;

            let resolved_base_url = match provider.as_str() {
                "deepseek" => "https://api.deepseek.com".to_string(),
                "openai" => "https://api.openai.com/v1".to_string(),
                "openrouter" => "https://openrouter.ai/api/v1".to_string(),
                _ => format!("https://api.{}.com", provider),
            };

            let resolved_model = match provider.as_str() {
                "deepseek" => "deepseek-chat".to_string(),
                "openai" => "gpt-4o".to_string(),
                "openrouter" => "deepseek/deepseek-chat".to_string(),
                _ => "deepseek-chat".to_string(),
            };

            (decrypted_key, resolved_base_url, resolved_model)
        }
        None => {
            // Fallback to environment variables
            let api_key = std::env::var("DEEPSEEK_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .or_else(|_| std::env::var("OPENROUTER_API_KEY"))
                .map_err(|_| anyhow::anyhow!("No AI API key configured"))?;

            let (base_url, model) = if std::env::var("DEEPSEEK_API_KEY").is_ok() {
                (
                    "https://api.deepseek.com".to_string(),
                    "deepseek-chat".to_string(),
                )
            } else if std::env::var("OPENAI_API_KEY").is_ok() {
                (
                    "https://api.openai.com/v1".to_string(),
                    "gpt-4o".to_string(),
                )
            } else {
                (
                    "https://openrouter.ai/api/v1".to_string(),
                    "deepseek/deepseek-chat".to_string(),
                )
            };

            (api_key, base_url, model)
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
