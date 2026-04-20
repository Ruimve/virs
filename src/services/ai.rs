use crate::config::AiConfig;
use crate::engine::lua_executor::{LuaExecutor, LuaExecutorConfig};
use serde::{Deserialize, Serialize};
use tracing::info;

const SYSTEM_PROMPT: &str = r#"You are a quantitative trading strategy developer for the VIRS platform.
You generate Lua strategy code that follows this exact format:

The user's strategy will be executed in a sandboxed Lua environment with these available functions and data:

**Available Functions:**
- sma(period): Simple Moving Average of close prices
- ema(period): Exponential Moving Average of close prices
- rsi(period): Relative Strength Index (0-100)

**Available Data:**
- klines: table of candle data with fields: open, high, low, close, volume, time
- current_idx: number (1-based index of current candle)
- params: table of user-defined parameters

**Requirements:**
1. The script MUST define a `function signal()` that returns: 1 (buy), -1 (sell), or 0 (hold)
2. Use `params.key_name` to read user parameters with sensible defaults via `or` operator
3. Keep logic concise and efficient
4. Add comments explaining the strategy logic
5. Only output the Lua code, no explanations outside the code
6. The code must be valid Lua 5.2 syntax

**Example output:**
```lua
-- EMA Crossover with RSI Filter
function signal()
  local fast = ema(params.fast_period or 12)
  local slow = ema(params.slow_period or 26)
  local rsi_val = rsi(params.rsi_period or 14)
  if fast > slow and rsi_val > 45 then
    return 1
  elseif fast < slow then
    return -1
  end
  return 0
end
```"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub prompt: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub code: String,
    pub name: String,
    pub description: String,
    pub params: Vec<ParamInfo>,
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

    /// Check if any AI provider is configured.
    pub fn is_configured(&self) -> bool {
        self.config.openrouter_api_key.is_some()
            || self.config.openai_api_key.is_some()
            || self.config.deepseek_api_key.is_some()
    }

    /// Get list of available providers.
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

    /// Generate a Lua strategy from natural language prompt.
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
            "Generate a Lua trading strategy based on this description:\n\n{}\n\n\
             Output ONLY the Lua code wrapped in ```lua ... ``` code blocks.",
            req.prompt
        );

        let request_body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user", "content": user_prompt }
            ],
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

        // Extract Lua code from markdown code blocks
        let code = extract_lua_code(&content);

        if code.trim().is_empty() {
            return Err(anyhow::anyhow!("AI did not generate valid Lua code"));
        }

        // Validate the generated code
        let executor = LuaExecutor::new(LuaExecutorConfig::default());
        if let Err(e) = executor.validate(&code) {
            return Err(anyhow::anyhow!(
                "Generated Lua code has syntax errors: {}",
                e
            ));
        }

        // Extract strategy name and description from comments
        let (name, description) = extract_metadata(&code);

        // Extract parameters from the code
        let params = extract_params(&code);

        info!(
            "AI generated strategy '{}' using {} ({})",
            name, provider, used_model
        );

        Ok(GenerateResponse {
            code,
            name,
            description,
            params,
            provider: provider.to_string(),
            model: used_model,
        })
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
                let model = requested_model.unwrap_or("deepseek-chat").to_string();
                Ok((key.clone(), "https://api.deepseek.com/v1".to_string(), model))
            }
            _ => Err(anyhow::anyhow!("Unknown AI provider: {}", provider)),
        }
    }
}

/// Extract Lua code from markdown code blocks.
fn extract_lua_code(content: &str) -> String {
    if let Some(start) = content.find("```lua") {
        let after_tag = start + 6;
        let code_begin = content[after_tag..]
            .find('\n')
            .map(|i| after_tag + i + 1)
            .unwrap_or(after_tag);
        if let Some(end) = content[code_begin..].find("```") {
            return content[code_begin..code_begin + end].trim().to_string();
        }
    }
    if let Some(start) = content.find("```") {
        let after = &content[start + 3..];
        let code_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        if let Some(end) = after[code_start..].find("```") {
            return after[code_start..code_start + end].trim().to_string();
        }
    }
    content.trim().to_string()
}

/// Extract strategy name and description from Lua comments.
fn extract_metadata(code: &str) -> (String, String) {
    let mut name = "AI Generated Strategy".to_string();
    let mut description = String::new();

    for line in code.lines().take(10) {
        let trimmed = line.trim();
        if !trimmed.starts_with("--") {
            break;
        }
        let comment = trimmed.trim_start_matches('-').trim();
        if name == "AI Generated Strategy" && !comment.is_empty() {
            // First non-empty comment is the name
            name = comment.to_string();
        } else if !comment.is_empty() {
            description = comment.to_string();
            break;
        }
    }

    (name, description)
}

/// Extract parameter definitions from params.xxx usage in the code.
fn extract_params(code: &str) -> Vec<ParamInfo> {
    let mut params = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for line in code.lines() {
        let trimmed = line.trim();

        // Match: params.fast_period or 12
        if let Some(pos) = trimmed.find("params.") {
            let rest = &trimmed[pos + 7..]; // skip "params."
            let param_name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !param_name.is_empty() && seen.insert(param_name.clone()) {
                let default_val = if let Some(or_pos) = rest.find(" or ") {
                    let after_or = &rest[or_pos + 4..];
                    after_or
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect::<String>()
                        .parse::<f64>()
                        .unwrap_or(0.0)
                } else {
                    0.0
                };

                let label = capitalize_words(&param_name.replace('_', " "));
                params.push(ParamInfo {
                    name: param_name,
                    label,
                    default: default_val,
                    min: None,
                    max: None,
                    step: None,
                });
            }
        }
    }

    params
}

/// Capitalize the first letter of each word.
fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
