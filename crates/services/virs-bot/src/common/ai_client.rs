//! LLM API client for bot AI services.

use tracing::{debug, warn};

pub struct LlmCallResult {
    pub content: serde_json::Value,
    pub used_model: String,
}

pub async fn call_llm_api(
    http_client: &reqwest::Client,
    api_key: &str,
    base_url: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    provider_name: &str,
) -> anyhow::Result<LlmCallResult> {
    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "response_format": { "type": "json_object" },
        "temperature": 0.5,
    });

    debug!(provider = %provider_name, model = %model, "Calling LLM API");

    let response = http_client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        anyhow::bail!("{} API returned {}: {}", provider_name, status, body_text);
    }

    let json: serde_json::Value = response.json().await?;

    let content_str = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if content_str.is_empty() {
        anyhow::bail!("AI returned empty response");
    }

    let used_model = json["model"].as_str().unwrap_or(model).to_string();

    let content: serde_json::Value = serde_json::from_str(&content_str).map_err(|e| {
        warn!("Failed to parse AI JSON response: {}, raw: {}", e, content_str);
        e
    })?;

    Ok(LlmCallResult { content, used_model })
}

pub fn create_llm_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}
