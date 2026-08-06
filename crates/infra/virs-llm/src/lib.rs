//! LLM HTTP 基础设施：封装 OpenAI 兼容的 `/chat/completions` 调用。
//!
//! 纯基础设施层，不依赖任何业务类型（无 virs-type）。
//! 所有需要调用 LLM 的 service crate 共享此实现。

use tracing::warn;
use virs_error::{BotError, BotResult};

pub struct LlmCallResult {
    pub content: serde_json::Value,
    pub used_model: String,
}

/// 调用 OpenAI 兼容的 LLM API（`POST {base_url}/chat/completions`）。
///
/// `provider_name` 仅用于错误日志标识调用方（如 "auto-ai"、"strategy-optimizer"）。
/// 返回的 `content` 已解析为 JSON Value。
pub async fn call_llm_api(
    http_client: &reqwest::Client,
    api_key: &str,
    base_url: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    provider_name: &str,
) -> BotResult<LlmCallResult> {
    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "response_format": { "type": "json_object" },
        "temperature": 0.5,
    });

    let response = http_client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_else(|e| {
            warn!(error = %e, "Failed to read LLM API error response body — using empty string");
            String::new()
        });
        return Err(BotError::llm(format!(
            "{} API returned {}: {}",
            provider_name, status, body_text
        )));
    }

    let json: serde_json::Value = response.json().await?;

    let content_str = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| {
            BotError::llm("LLM response missing 'choices[0].message.content'".to_string())
        })?
        .to_string();

    if content_str.is_empty() {
        return Err(BotError::llm("AI returned empty response".to_string()));
    }

    let used_model = json["model"].as_str().unwrap_or(model).to_string();

    let content: serde_json::Value = serde_json::from_str(&content_str).map_err(|e| {
        warn!(
            error = %e,
            raw = %content_str,
            "Failed to parse AI JSON response"
        );
        BotError::llm(e.to_string())
    })?;

    Ok(LlmCallResult {
        content,
        used_model,
    })
}

/// 创建带超时控制的 LLM HTTP 客户端。
///
/// 连接超时固定 10s，请求总超时由 `llm_timeout` 控制。
pub fn create_llm_http_client(llm_timeout: std::time::Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(llm_timeout)
        .build()
        .unwrap_or_else(|_| {
            warn!("LLM HTTP client builder failed — creating fallback with timeout only");
            reqwest::Client::builder()
                .timeout(llm_timeout)
                .build()
                .expect("fallback client with timeout must succeed")
        })
}
