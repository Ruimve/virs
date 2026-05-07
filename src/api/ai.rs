use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::middleware::AuthUser;
use crate::api::AppState;
use crate::indicators;
use crate::models::*;
use crate::services::ai::{AiService, AiUserConfig, GenerateRequest};
use crate::utils::crypto;

/// Load user AI credentials from the database and decrypt them.
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

/// Check AI service availability and list providers.
pub async fn ai_status(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Json<ApiResponse<serde_json::Value>> {
    let ai_service = AiService::new(state.config.ai.clone());

    // Load user-specific AI credentials
    let user_id = auth.uuid().unwrap_or(Uuid::nil());
    let encryption_key = crypto::derive_key(&state.config.server.encryption_key);
    let user_config = load_user_ai_config(&state.db_pool, &user_id, &encryption_key).await;

    // User-configured providers
    let mut user_providers = Vec::new();
    if user_config.openrouter_api_key.is_some() {
        user_providers.push("openrouter".to_string());
    }
    if user_config.openai_api_key.is_some() {
        user_providers.push("openai".to_string());
    }
    if user_config.deepseek_api_key.is_some() {
        user_providers.push("deepseek".to_string());
    }

    Json(ApiResponse::ok(serde_json::json!({
        "configured": ai_service.is_configured_with_override(&user_config),
        "system_providers": ai_service.available_providers(),
        "user_providers": user_providers,
    })))
}

/// Generate a trading strategy from natural language.
pub async fn generate_strategy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<GenerateRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    if body.prompt.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("Prompt cannot be empty")),
        ));
    }

    let ai_service = AiService::new(state.config.ai.clone());

    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;
    let encryption_key = crypto::derive_key(&state.config.server.encryption_key);
    let user_config = load_user_ai_config(&state.db_pool, &user_id, &encryption_key).await;

    if !ai_service.is_configured_with_override(&user_config) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<serde_json::Value>::err(
                "No AI provider configured. Set OPENROUTER_API_KEY, OPENAI_API_KEY, or DEEPSEEK_API_KEY in .env, or configure user-level AI credentials.",
            )),
        ));
    }

    let provider = body
        .provider
        .as_deref()
        .unwrap_or_else(|| ai_service.default_provider_with_override(&user_config));

    let (api_key, base_url, model) = ai_service
        .resolve_provider_with_override(provider, body.model.as_deref(), &user_config)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(format!("{}", e))),
            )
        })?;

    let user_prompt = format!(
        "Generate a trading strategy based on this description:\n\n{}\n\n\
         Output the strategy as a JSON object with fields: name, description, params (array of {{name, label, default, min, max}}), and rules (object with entry_conditions, exit_conditions, risk_management).",
        body.prompt
    );

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": "You are a quantitative trading strategy developer. Generate strategies as structured JSON. Output ONLY valid JSON, no markdown code blocks." },
            { "role": "user", "content": user_prompt }
        ],
        "response_format": { "type": "json_object" },
        "temperature": 0.7,
        "max_tokens": 2000,
    });

    let client = &state.http_client;
    let response = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("AI strategy generation request failed: {}", e);
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

    let result: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        tracing::error!("Failed to parse AI strategy JSON: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "AI returned invalid JSON: {}",
                e
            ))),
        )
    })?;

    tracing::info!(
        "AI generated strategy using {} ({})",
        provider, used_model
    );

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "strategy": result,
        "provider": provider,
        "model": used_model,
    }))))
}

#[derive(Deserialize)]
pub struct OptimizeRequest {
    pub strategy_code: Option<String>,
    pub backtest_summary: serde_json::Value,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Deserialize)]
pub struct ExplainRequest {
    pub strategy_code: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

/// AI parameter optimization suggestions based on backtest results.
pub async fn optimize(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<OptimizeRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let ai_service = AiService::new(state.config.ai.clone());

    // Load user AI credentials
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;
    let encryption_key = crypto::derive_key(&state.config.server.encryption_key);
    let user_config = load_user_ai_config(&state.db_pool, &user_id, &encryption_key).await;

    if !ai_service.is_configured_with_override(&user_config) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err(
                "AI service is not configured",
            )),
        ));
    }

    let provider = body
        .provider
        .as_deref()
        .unwrap_or_else(|| ai_service.default_provider_with_override(&user_config));

    let (api_key, base_url, model) = ai_service
        .resolve_provider_with_override(provider, body.model.as_deref(), &user_config)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(format!("{}", e))),
            )
        })?;

    let system_prompt = r#"你是一位量化交易策略优化专家，服务于 VIRS 平台。
分析回测结果，并给出具体的参数调整建议以改善策略表现。

重点关注：
1. 提高胜率（目标 > 55%）
2. 改善风险调整后收益（夏普比率 > 1.0）
3. 降低最大回撤（目标 < 15%）
4. 改善盈亏比（目标 > 1.5）
5. 优化交易频率

对每条建议，请提供：
- 当前值和建议值
- 调整理由
- 预期影响

如果用户使用中文，请用中文回复；如果使用英文，请用英文回复。
保持回复简洁、可操作。使用 markdown 格式。"#;

    let code_section = match body.strategy_code.as_deref() {
        Some(code) if !code.trim().is_empty() => {
            format!("以下是策略代码：\n```lua\n{}\n```\n\n", code)
        }
        _ => String::new(),
    };

    let user_prompt = format!(
        "{}以下是回测结果：\n```json\n{}\n```\n\n请分析并给出具体的参数优化建议。",
        code_section,
        serde_json::to_string_pretty(&body.backtest_summary).unwrap_or_default()
    );

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "temperature": 0.5,
        "max_tokens": 2000,
    });

    let client = &state.http_client;
    let response = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("AI optimization request failed: {}", e);
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

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "suggestion": content,
    }))))
}

/// AI strategy explanation.
pub async fn explain(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<ExplainRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    let ai_service = AiService::new(state.config.ai.clone());

    // Load user AI credentials
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;
    let encryption_key = crypto::derive_key(&state.config.server.encryption_key);
    let user_config = load_user_ai_config(&state.db_pool, &user_id, &encryption_key).await;

    if !ai_service.is_configured_with_override(&user_config) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err(
                "AI service is not configured",
            )),
        ));
    }

    let provider = body
        .provider
        .as_deref()
        .unwrap_or_else(|| ai_service.default_provider_with_override(&user_config));

    let (api_key, base_url, model) = ai_service
        .resolve_provider_with_override(provider, body.model.as_deref(), &user_config)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(format!("{}", e))),
            )
        })?;

    let system_prompt = r#"你是一位量化交易策略分析师，服务于 VIRS 平台。
用清晰、简洁的自然语言解释给定的 Lua 策略代码。

你的解释应包括：
1. **策略概述**：这是什么类型的策略？（趋势跟踪、均值回归、动量等）
2. **入场逻辑**：何时买入/卖出？
3. **使用的指标**：使用了哪些技术指标，为什么？
4. **参数说明**：每个参数控制什么，其效果如何
5. **优势**：该策略在什么市场条件下表现最佳
6. **风险**：潜在的弱点或失败模式

如果策略注释使用中文，请用中文回复；如果使用英文，请用英文回复。
保持回复简洁。使用 markdown 格式。"#;

    let user_prompt = format!(
        "请解释以下交易策略：\n```lua\n{}\n```",
        body.strategy_code
    );

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "temperature": 0.5,
        "max_tokens": 1500,
    });

    let client = &state.http_client;
    let response = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("AI explanation request failed: {}", e);
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

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "explanation": content,
    }))))
}

#[derive(Deserialize)]
pub struct RecommendRequest {
    pub symbol: String,
    pub exchange: String,
    pub timeframe: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

/// AI strategy recommendation based on market analysis.
pub async fn recommend_strategy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<RecommendRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<serde_json::Value>>)>
{
    if req.symbol.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("symbol must not be empty")),
        ));
    }

    if req.timeframe.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::err("timeframe must not be empty")),
        ));
    }

    // --- Step 1: Get exchange instance ---
    let exchange_key = super::market::ensure_exchange(&state, &req.exchange, MarketType::Spot).await?;
    let exchange = state.exchange_registry.get(&exchange_key).unwrap();

    // --- Step 2: Fetch recent klines (up to 200) ---
    let now_ms = chrono::Utc::now().timestamp_millis();
    let start_ms = now_ms - 200 * 24 * 3600 * 1000; // enough to get 200 candles

    let klines = match exchange.get_klines_range(&req.symbol, &req.timeframe, start_ms, now_ms).await {
        Ok(k) if k.len() >= 50 => k,
        Ok(k) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "Insufficient kline data: got {} candles, need at least 50. Please verify the symbol and timeframe.",
                    k.len()
                ))),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                Json(ApiResponse::<serde_json::Value>::err(format!(
                    "Failed to fetch klines for {} from {}: {}",
                    req.symbol, req.exchange, e
                ))),
            ));
        }
    };

    // --- Step 3: Build kline data summary ---
    let last_30: &[Kline] = if klines.len() >= 30 {
        &klines[klines.len() - 30..]
    } else {
        &klines
    };

    // Build OHLCV table for the last 30 candles
    let mut ohlcv_table = String::from("Time,Open,High,Low,Close,Volume\n");
    for k in last_30.iter() {
        let time_str = chrono::DateTime::from_timestamp_millis(k.open_time)
            .map(|dt| dt.format("%m-%d %H:%M").to_string())
            .unwrap_or_else(|| k.open_time.to_string());
        ohlcv_table.push_str(&format!(
            "{},{:.2},{:.2},{:.2},{:.2},{:.2}\n",
            time_str, k.open, k.high, k.low, k.close, k.volume
        ));
    }

    // Compute key indicators
    let last_close = klines.last().map(|k| k.close).unwrap_or(0.0);
    let first_close_30 = last_30.first().map(|k| k.close).unwrap_or(last_close);
    let change_pct = if first_close_30 > 0.0 {
        (last_close - first_close_30) / first_close_30 * 100.0
    } else {
        0.0
    };

    let high_30: f64 = last_30.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max);
    let low_30: f64 = last_30.iter().map(|k| k.low).fold(f64::INFINITY, f64::min);
    let avg_close: f64 = last_30.iter().map(|k| k.close).sum::<f64>() / last_30.len() as f64;

    // RSI(14)
    let rsi_val = indicators::rsi_at(&klines, klines.len() - 1, 14);

    // ATR(14)
    let atr_val = indicators::atr_at(&klines, klines.len() - 1, 14);

    // EMA(12) and EMA(26)
    let ema12_val = indicators::ema_at(&klines, klines.len() - 1, 12);
    let ema26_val = indicators::ema_at(&klines, klines.len() - 1, 26);
    let ema12_prev = indicators::ema_at(&klines, klines.len().saturating_sub(2), 12);
    let ema26_prev = indicators::ema_at(&klines, klines.len().saturating_sub(2), 26);

    let ema12_trend = if ema12_val > ema12_prev { "up" } else if ema12_val < ema12_prev { "down" } else { "flat" };
    let ema26_trend = if ema26_val > ema26_prev { "up" } else if ema26_val < ema26_prev { "down" } else { "flat" };

    // --- Step 4: Build prompt ---
    let user_prompt = format!(
        r#"你是一个专业的加密货币量化交易策略分析师。根据以下市场数据分析当前市场状态，并推荐最合适的 1-3 个交易策略。

## 市场数据
交易对: {symbol}
周期: {timeframe}
最近30根K线:
{ohlcv_table}

## 关键指标
RSI(14): {rsi:.2}
ATR(14): {atr:.2}
EMA(12): {ema12:.2} (方向: {ema12_trend})
EMA(26): {ema26:.2} (方向: {ema26_trend})
价格区间: {low:.2} - {high:.2}
均价: {avg:.2}
近30根涨跌幅: {change_pct:.2}%

## 输出要求
输出 JSON 格式（不要 markdown 代码块）：
{{
  "market_analysis": {{
    "regime": "trending_up|trending_down|ranging|volatile",
    "volatility": "low|medium|high",
    "summary": "一句话描述市场状态"
  }},
  "recommendations": [
    {{
      "rank": 1,
      "strategy_type": "grid|dca|momentum|mean_reversion",
      "confidence": 0.0-1.0,
      "reason": "推荐理由",
      "params": {{ "参数名": 值 }}
    }}
  ]
}}"#,
        symbol = req.symbol,
        timeframe = req.timeframe,
        ohlcv_table = ohlcv_table,
        rsi = rsi_val,
        atr = atr_val,
        ema12 = ema12_val,
        ema12_trend = ema12_trend,
        ema26 = ema26_val,
        ema26_trend = ema26_trend,
        low = low_30,
        high = high_30,
        avg = avg_close,
        change_pct = change_pct,
    );

    // --- Step 6: Call AI API ---
    let ai_service = AiService::new(state.config.ai.clone());

    // Load user AI credentials
    let user_id = auth.uuid().map_err(|e| (StatusCode::UNAUTHORIZED, Json(ApiResponse::<serde_json::Value>::err(&e))))?;
    let encryption_key = crypto::derive_key(&state.config.server.encryption_key);
    let user_config = load_user_ai_config(&state.db_pool, &user_id, &encryption_key).await;

    if !ai_service.is_configured_with_override(&user_config) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<serde_json::Value>::err(
                "No AI provider configured. Set OPENROUTER_API_KEY, OPENAI_API_KEY, or DEEPSEEK_API_KEY in .env, or configure user-level AI credentials.",
            )),
        ));
    }

    let provider = req
        .provider
        .as_deref()
        .unwrap_or_else(|| ai_service.default_provider_with_override(&user_config));

    let (api_key, base_url, model) = ai_service
        .resolve_provider_with_override(provider, req.model.as_deref(), &user_config)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value>::err(format!("{}", e))),
            )
        })?;

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": "你是一个专业的加密货币量化交易策略分析师。你只输出 JSON，不输出 markdown 代码块或其他格式。" },
            { "role": "user", "content": user_prompt }
        ],
        "temperature": 0.5,
        "max_tokens": 2000,
    });

    let client = &state.http_client;
    let response = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("AI recommend strategy request failed: {}", e);
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

    // --- Step 7: Parse JSON response ---
    // Strip markdown code blocks if present
    let content = content.trim();
    let content = if content.starts_with("```json") {
        content.trim_start_matches("```json").trim_end_matches("```").trim()
    } else if content.starts_with("```") {
        content.trim_start_matches("```").trim_end_matches("```").trim()
    } else {
        content
    };

    let result: serde_json::Value = serde_json::from_str(content).map_err(|e| {
        tracing::error!("Failed to parse AI recommendation JSON: {}", e);
        tracing::error!("Raw content: {}", content);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::err(format!(
                "AI returned invalid JSON: {}. Raw response: {}",
                e, content
            ))),
        )
    })?;

    tracing::info!(
        "AI recommended strategy for {} using {} ({})",
        req.symbol, provider, json["model"].as_str().unwrap_or(&model)
    );

    Ok(Json(ApiResponse::ok(result)))
}


