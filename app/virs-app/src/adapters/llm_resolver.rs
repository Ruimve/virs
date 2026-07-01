//! DefaultLlmResolver — resolves LLM provider, API key, base URL, and model.

use virs_config::AiConfig;
use virs_types::bot::{BotError, BotResult, LlmProviderResolver};

/// Resolve LLM provider info from user credentials and AI config.
///
/// Priority: deepseek > openai > openrouter.
/// User credentials take precedence over system config keys.
///
/// Returns (api_key, base_url, model, provider_name).
pub fn resolve_llm_provider(
    user_credentials: &[(String, String, Option<String>)],
    ai_config: &AiConfig,
) -> BotResult<(String, String, String, String)> {
    // Check user credentials first
    let mut user_deepseek = None;
    let mut user_openai = None;
    let mut user_openrouter = None;

    for (provider, key, model) in user_credentials {
        match provider.as_str() {
            "deepseek" => user_deepseek = Some((key.clone(), model.clone())),
            "openai" => user_openai = Some((key.clone(), model.clone())),
            "openrouter" => user_openrouter = Some((key.clone(), model.clone())),
            _ => {}
        }
    }

    // Priority: deepseek > openai > openrouter
    if let Some((key, model)) =
        user_deepseek.or(ai_config.deepseek_api_key.clone().map(|k| (k, None)))
    {
        let model = model.unwrap_or_else(|| "deepseek-chat".to_string());
        return Ok((
            key,
            "https://api.deepseek.com".to_string(),
            model,
            "deepseek".to_string(),
        ));
    }

    if let Some((key, model)) =
        user_openai.or(ai_config.openai_api_key.clone().map(|k| (k, None)))
    {
        let model = model.unwrap_or_else(|| "gpt-4o".to_string());
        return Ok((
            key,
            "https://api.openai.com/v1".to_string(),
            model,
            "openai".to_string(),
        ));
    }

    if let Some((key, model)) =
        user_openrouter.or(ai_config.openrouter_api_key.clone().map(|k| (k, None)))
    {
        let model = model.unwrap_or_else(|| "deepseek/deepseek-chat".to_string());
        return Ok((
            key,
            "https://openrouter.ai/api/v1".to_string(),
            model,
            "openrouter".to_string(),
        ));
    }

    Err(BotError::Llm("No LLM API key configured. Set DEEPSEEK_API_KEY, OPENAI_API_KEY, or OPENROUTER_API_KEY".to_string()))
}

pub struct DefaultLlmResolver {
    ai_config: AiConfig,
}

impl DefaultLlmResolver {
    pub fn new(ai_config: AiConfig) -> Self {
        Self { ai_config }
    }
}

impl LlmProviderResolver for DefaultLlmResolver {
    fn is_available(&self) -> bool {
        self.ai_config.deepseek_api_key.is_some()
            || self.ai_config.openai_api_key.is_some()
            || self.ai_config.openrouter_api_key.is_some()
    }

    fn resolve(
        &self,
        user_credentials: &[(String, String, Option<String>)],
    ) -> BotResult<(String, String, String, String)> {
        resolve_llm_provider(user_credentials, &self.ai_config)
    }
}
