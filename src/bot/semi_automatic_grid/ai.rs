//! 网格机器人 LLM 决策服务
//!
//! 为 GridWorker 提供运行时 LLM 决策能力，复用 AiService 的 provider 解析逻辑。

use serde::Deserialize;
use sqlx::PgPool;
use tracing::{debug, warn};

use crate::config::AiConfig;
use crate::services::ai::{AiService, AiUserConfig};

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

/// 网格 AI 服务，为 GridWorker 提供独立的 LLM 调用能力
pub struct GridAiService {
    ai_service: AiService,
    ai_config: AiConfig,
    http_client: reqwest::Client,
    db: PgPool,
    encryption_key: [u8; 32],
}

impl GridAiService {
    pub fn new(ai_config: AiConfig, db: PgPool, encryption_key: [u8; 32]) -> Self {
        let http_client = reqwest::Client::new();
        let ai_service = AiService::new(ai_config.clone());
        Self {
            ai_service,
            ai_config,
            http_client,
            db,
            encryption_key,
        }
    }

    /// 检查是否有可用的 AI provider
    pub fn is_available(&self) -> bool {
        let config = AiUserConfig::default();
        self.ai_service.is_configured_with_override(&config)
    }

    /// 获取默认 provider 名称
    pub fn default_provider(&self) -> Option<&'static str> {
        if self.ai_config.openrouter_api_key.is_some() {
            Some("openrouter")
        } else if self.ai_config.openai_api_key.is_some() {
            Some("openai")
        } else if self.ai_config.deepseek_api_key.is_some() {
            Some("deepseek")
        } else {
            None
        }
    }

    /// 调用 LLM 并返回解析后的 JSON
    pub async fn call_llm(
        &self,
        user_id: &uuid::Uuid,
        system_prompt: &str,
        user_prompt: &str,
    ) -> anyhow::Result<serde_json::Value> {
        // 尝试加载用户级 AI 凭证
        let user_config = self.load_user_ai_config(user_id).await;
        let default_config = AiUserConfig::default();

        if !self.ai_service.is_configured_with_override(&user_config)
            && !self.ai_service.is_configured_with_override(&default_config)
        {
            anyhow::bail!("No AI provider configured");
        }

        // 优先使用用户凭证，回退到系统默认
        let effective_config = if self.ai_service.is_configured_with_override(&user_config) {
            user_config
        } else {
            default_config
        };

        let provider = self
            .ai_service
            .default_provider_with_override(&effective_config);

        let (api_key, base_url, model) = self
            .ai_service
            .resolve_provider_with_override(&provider, None, &effective_config)?;

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
    ///
    /// 返回 `GridDecision`，包含 action 和 reason。
    /// 如果 LLM 不可用或解析失败，返回 None（调用方应回退到规则决策）。
    pub async fn grid_decision(
        &self,
        user_id: &uuid::Uuid,
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

    /// 从数据库加载用户级 AI 凭证
    async fn load_user_ai_config(&self, user_id: &uuid::Uuid) -> AiUserConfig {
        #[derive(Debug, sqlx::FromRow)]
        struct EncryptedRow {
            pub provider: String,
            pub encrypted_api_key: String,
        }

        let rows = sqlx::query_as::<_, EncryptedRow>(
            r#"SELECT provider, encrypted_api_key FROM qd_ai_credentials WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await;

        let mut config = AiUserConfig::default();

        if let Ok(rows) = rows {
            for row in rows {
                let decrypted = match crate::utils::crypto::decrypt(&row.encrypted_api_key, &self.encryption_key) {
                    Ok(key) => key,
                    Err(e) => {
                        warn!(provider = %row.provider, "Failed to decrypt user AI key: {}", e);
                        continue;
                    }
                };

                match row.provider.as_str() {
                    "openrouter" => config.openrouter_api_key = Some(decrypted),
                    "openai" => config.openai_api_key = Some(decrypted),
                    "deepseek" => config.deepseek_api_key = Some(decrypted),
                    _ => {}
                }
            }
        }

        config
    }
}
