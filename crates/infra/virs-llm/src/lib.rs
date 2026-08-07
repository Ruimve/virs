

use tracing::warn;
use virs_error::{BotError, BotResult};

/* LLM 调用结果：包含解析后的 JSON 内容和实际使用的模型名称 */
pub struct LlmCallResult {
    pub content: serde_json::Value,
    pub used_model: String,
}


/*
 * 调用 LLM Chat Completions API：构造标准请求并解析响应。
 * 使用 temperature=0.5 和 JSON 响应格式，确保输出可被程序解析。
 * 返回解析后的 JSON 内容和实际使用的模型名称。
 */
pub async fn call_llm_api(
    http_client: &reqwest::Client,
    api_key: &str,
    base_url: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    provider_name: &str,
) -> BotResult<LlmCallResult> {
    /* 构造 Chat Completions 请求：system+user 双消息，强制 JSON 输出 */
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

    /* 提取 choices[0].message.content：标准 OpenAI 兼容响应格式 */
    let content_str = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| {
            BotError::llm("LLM response missing 'choices[0].message.content'".to_string())
        })?
        .to_string();

    if content_str.is_empty() {
        return Err(BotError::llm("AI returned empty response".to_string()));
    }

    /* 实际使用的模型：优先从响应中提取，回退到请求模型名 */
    let used_model = json["model"].as_str().unwrap_or(model).to_string();

    /* 将 content 字符串解析为 JSON：LLM 返回的 content 是 JSON 字符串，需二次解析 */
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


/*
 * 创建 LLM 专用 HTTP 客户端：设置连接超时（10秒）和总超时。
 * 构建失败时回退到仅设置总超时的客户端。
 */
pub fn create_llm_http_client(llm_timeout: std::time::Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(llm_timeout)
        .build()
        .unwrap_or_else(|_| {
            /* 构建失败时回退：仅设置总超时，不设置连接超时 */
            warn!("LLM HTTP client builder failed — creating fallback with timeout only");
            reqwest::Client::builder()
                .timeout(llm_timeout)
                .build()
                .expect("fallback client with timeout must succeed")
        })
}
