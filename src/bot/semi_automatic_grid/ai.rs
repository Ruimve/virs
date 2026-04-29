//! 网格机器人 LLM 决策服务
//!
//! 通过 trait 抽象实现业务隔离，不直接依赖外部模块。

use serde::Deserialize;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ports::{CredentialStore, LlmProviderResolver};

/// LLM 决策返回的 action
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridAction {
    RunGrid,
    PauseGrid,
    AdjustGrid {
        upper_price: Option<f64>,
        lower_price: Option<f64>,
    },
    ReducePosition,
    Hold,
}

impl GridAction {
    pub fn as_str(&self) -> &str {
        match self {
            Self::RunGrid => "run_grid",
            Self::PauseGrid => "pause_grid",
            Self::AdjustGrid { .. } => "adjust_grid",
            Self::ReducePosition => "reduce_position",
            Self::Hold => "hold",
        }
    }
}

/// LLM 决策的完整结果
#[derive(Debug, Clone, Deserialize)]
pub struct GridDecision {
    pub action: GridAction,
    pub reason: String,
    #[serde(default)]
    pub upper_price: Option<f64>,
    #[serde(default)]
    pub lower_price: Option<f64>,
}

/// 网格 AI 服务
pub struct GridAiService {
    resolver: Box<dyn LlmProviderResolver>,
    credential_store: Box<dyn CredentialStore>,
    http_client: reqwest::Client,
}

impl GridAiService {
    pub fn new(
        resolver: Box<dyn LlmProviderResolver>,
        credential_store: Box<dyn CredentialStore>,
    ) -> Self {
        Self {
            resolver,
            credential_store,
            http_client: reqwest::Client::new(),
        }
    }

    /// 检查是否有可用的 AI provider
    pub fn is_available(&self) -> bool {
        self.resolver.is_available()
    }

    /// 调用 LLM 并返回解析后的 JSON
    pub async fn call_llm(
        &self,
        user_id: &Uuid,
        system_prompt: &str,
        user_prompt: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let user_creds = self.credential_store.load_credentials(*user_id).await?;

        let (api_key, base_url, model, provider) =
            self.resolver.resolve(&user_creds)?;

        let request_body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ],
            "response_format": { "type": "json_object" },
            "temperature": 0.5,
        });

        debug!(
            provider = %provider,
            model = %model,
            "Calling LLM for grid decision"
        );

        let response = self
            .http_client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            anyhow::bail!("{} API returned {}: {}", provider, status, body_text);
        }

        let json: serde_json::Value = response.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if content.is_empty() {
            anyhow::bail!("AI returned empty response");
        }

        let result: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            warn!("Failed to parse AI JSON response: {}, raw: {}", e, content);
            e
        })?;

        Ok(result)
    }

    /// 调用 LLM 进行网格决策
    pub async fn grid_decision(
        &self,
        user_id: &Uuid,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Option<GridDecision> {
        match self.call_llm(user_id, system_prompt, user_prompt).await {
            Ok(json) => {
                let action_str = json["action"].as_str().unwrap_or("hold");
                let reason = json["reason"]
                    .as_str()
                    .unwrap_or("No reason provided")
                    .to_string();
                let upper_price = json["upper_price"].as_f64();
                let lower_price = json["lower_price"].as_f64();

                let action = match action_str {
                    "run_grid" => GridAction::RunGrid,
                    "pause_grid" => GridAction::PauseGrid,
                    "adjust_grid" => GridAction::AdjustGrid {
                        upper_price,
                        lower_price,
                    },
                    "reduce_position" => GridAction::ReducePosition,
                    _ => GridAction::Hold,
                };

                Some(GridDecision {
                    action,
                    reason,
                    upper_price,
                    lower_price,
                })
            }
            Err(e) => {
                warn!("LLM grid decision failed, falling back to rules: {}", e);
                None
            }
        }
    }
}
