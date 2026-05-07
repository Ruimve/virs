use crate::config::AiConfig;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Default)]
pub struct AiUserConfig {
    pub openrouter_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub deepseek_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub prompt: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub strategy: serde_json::Value,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamInfo {
    pub name: String,
    pub label: String,
    pub default: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
}

pub struct AiService {
    config: AiConfig,
    client: reqwest::Client,
}

impl AiService {
    pub fn new(config: AiConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.config.openrouter_api_key.is_some()
            || self.config.openai_api_key.is_some()
            || self.config.deepseek_api_key.is_some()
    }

    pub fn available_providers(&self) -> Vec<String> {
        let mut providers = Vec::new();
        if self.config.openrouter_api_key.is_some() {
            providers.push("openrouter".into());
        }
        if self.config.openai_api_key.is_some() {
            providers.push("openai".into());
        }
        if self.config.deepseek_api_key.is_some() {
            providers.push("deepseek".into());
        }
        providers
    }

    pub async fn generate_strategy(
        &self,
        req: &GenerateRequest,
    ) -> anyhow::Result<GenerateResponse> {
        let provider = req
            .provider
            .as_deref()
            .unwrap_or_else(|| self.default_provider());

        let (api_key, base_url, model) = self.resolve_provider(provider, req.model.as_deref())?;

        let user_prompt = format!(
            "Generate a trading strategy based on this description:\n\n{}\n\n\
             Output the strategy as a JSON object with fields: name, description, params (array of {{name, label, default, min, max}}), and rules (object with entry_conditions, exit_conditions, risk_management).",
            req.prompt
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

        let response = self
            .client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to call {} API: {}", provider, e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("{} API returned {}: {}", provider, status, body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse {} response: {}", provider, e))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let used_model = json["model"].as_str().unwrap_or(&model).to_string();

        let result: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("AI returned invalid JSON: {}", e))?;

        info!(
            "AI generated strategy using {} ({})",
            provider, used_model
        );

        Ok(GenerateResponse {
            strategy: result,
            provider: provider.to_string(),
            model: used_model,
        })
    }

    pub async fn optimize_strategy(
        &self,
        strategy_code: Option<&str>,
        backtest_summary: &serde_json::Value,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> anyhow::Result<String> {
        let provider = provider.unwrap_or_else(|| self.default_provider());
        let (api_key, base_url, model) = self.resolve_provider(provider, model)?;

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

        let code_section = match strategy_code {
            Some(code) if !code.trim().is_empty() => {
                format!("以下是策略代码：\n```\n{}\n```\n\n", code)
            }
            _ => String::new(),
        };

        let user_prompt = format!(
            "{}以下是回测结果：\n```json\n{}\n```\n\n请分析并给出具体的参数优化建议。",
            code_section,
            serde_json::to_string_pretty(backtest_summary).unwrap_or_default()
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

        let response = self
            .client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to call {} API: {}", provider, e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("{} API returned {}: {}", provider, status, body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse {} response: {}", provider, e))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }

    pub async fn explain_strategy(
        &self,
        strategy_code: &str,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> anyhow::Result<String> {
        let provider = provider.unwrap_or_else(|| self.default_provider());
        let (api_key, base_url, model) = self.resolve_provider(provider, model)?;

        let system_prompt = r#"你是一位量化交易策略分析师，服务于 VIRS 平台。
用清晰、简洁的自然语言解释给定的交易策略。

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
            "请解释以下交易策略：\n```\n{}\n```",
            strategy_code
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

        let response = self
            .client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to call {} API: {}", provider, e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("{} API returned {}: {}", provider, status, body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse {} response: {}", provider, e))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }

    fn default_provider(&self) -> &'static str {
        if self.config.openrouter_api_key.is_some() {
            "openrouter"
        } else if self.config.openai_api_key.is_some() {
            "openai"
        } else if self.config.deepseek_api_key.is_some() {
            "deepseek"
        } else {
            "openrouter"
        }
    }

    fn resolve_provider(
        &self,
        provider: &str,
        requested_model: Option<&str>,
    ) -> anyhow::Result<(String, String, String)> {
        match provider {
            "openrouter" => {
                let key = self.config.openrouter_api_key.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("OPENROUTER_API_KEY is not configured")
                })?;
                let model = requested_model.unwrap_or("google/gemini-2.0-flash-001").to_string();
                Ok((key.clone(), "https://openrouter.ai/api/v1".to_string(), model))
            }
            "openai" => {
                let key = self.config.openai_api_key.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("OPENAI_API_KEY is not configured"))?;
                let model = requested_model.unwrap_or("gpt-4o-mini").to_string();
                Ok((key.clone(), "https://api.openai.com/v1".to_string(), model))
            }
            "deepseek" => {
                let key = self.config.deepseek_api_key.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("DEEPSEEK_API_KEY is not configured"))?;
                let model = requested_model.unwrap_or("deepseek-v4-flash").to_string();
                Ok((key.clone(), "https://api.deepseek.com/v1".to_string(), model))
            }
            _ => Err(anyhow::anyhow!("Unknown AI provider: {}", provider)),
        }
    }

    pub fn resolve_provider_with_override(
        &self,
        provider: &str,
        requested_model: Option<&str>,
        user_config: &AiUserConfig,
    ) -> anyhow::Result<(String, String, String)> {
        match provider {
            "openrouter" => {
                let key = user_config
                    .openrouter_api_key
                    .as_ref()
                    .or(self.config.openrouter_api_key.as_ref())
                    .ok_or_else(|| anyhow::anyhow!("OPENROUTER_API_KEY is not configured (neither user nor system)"))?;
                let model = requested_model.unwrap_or("google/gemini-2.0-flash-001").to_string();
                Ok((key.clone(), "https://openrouter.ai/api/v1".to_string(), model))
            }
            "openai" => {
                let key = user_config
                    .openai_api_key
                    .as_ref()
                    .or(self.config.openai_api_key.as_ref())
                    .ok_or_else(|| anyhow::anyhow!("OPENAI_API_KEY is not configured (neither user nor system)"))?;
                let model = requested_model.unwrap_or("gpt-4o-mini").to_string();
                Ok((key.clone(), "https://api.openai.com/v1".to_string(), model))
            }
            "deepseek" => {
                let key = user_config
                    .deepseek_api_key
                    .as_ref()
                    .or(self.config.deepseek_api_key.as_ref())
                    .ok_or_else(|| anyhow::anyhow!("DEEPSEEK_API_KEY is not configured (neither user nor system)"))?;
                let model = requested_model.unwrap_or("deepseek-v4-flash").to_string();
                Ok((key.clone(), "https://api.deepseek.com/v1".to_string(), model))
            }
            _ => Err(anyhow::anyhow!("Unknown AI provider: {}", provider)),
        }
    }

    pub fn is_configured_with_override(&self, user_config: &AiUserConfig) -> bool {
        user_config.openrouter_api_key.is_some()
            || user_config.openai_api_key.is_some()
            || user_config.deepseek_api_key.is_some()
            || self.is_configured()
    }

    pub fn default_provider_with_override(&self, user_config: &AiUserConfig) -> &'static str {
        if user_config.openrouter_api_key.is_some() {
            "openrouter"
        } else if user_config.openai_api_key.is_some() {
            "openai"
        } else if user_config.deepseek_api_key.is_some() {
            "deepseek"
        } else {
            self.default_provider()
        }
    }
}
