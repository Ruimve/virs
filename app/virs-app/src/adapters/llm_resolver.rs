//! DefaultLlmResolver — resolves LLM provider, API key, base URL, and model.

use virs_config::AiConfig;
use virs_types::bot::{LlmProviderResolver, BotError, BotResult};

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
        user_credentials: &[(String, String)],
    ) -> BotResult<(String, String, String, String)> {
        // Check user credentials first
        let mut user_deepseek = None;
        let mut user_openai = None;
        let mut user_openrouter = None;

        for (provider, key) in user_credentials {
            match provider.as_str() {
                "deepseek" => user_deepseek = Some(key.clone()),
                "openai" => user_openai = Some(key.clone()),
                "openrouter" => user_openrouter = Some(key.clone()),
                _ => {}
            }
        }

        // Priority: deepseek > openai > openrouter
        if let Some(key) = user_deepseek.or(self.ai_config.deepseek_api_key.clone()) {
            return Ok((
                key,
                "https://api.deepseek.com".to_string(),
                "deepseek-chat".to_string(),
                "deepseek".to_string(),
            ));
        }

        if let Some(key) = user_openai.or(self.ai_config.openai_api_key.clone()) {
            return Ok((
                key,
                "https://api.openai.com/v1".to_string(),
                "gpt-4o".to_string(),
                "openai".to_string(),
            ));
        }

        if let Some(key) = user_openrouter.or(self.ai_config.openrouter_api_key.clone()) {
            return Ok((
                key,
                "https://openrouter.ai/api/v1".to_string(),
                "deepseek/deepseek-chat".to_string(),
                "openrouter".to_string(),
            ));
        }

        Err(BotError::Llm("No LLM API key configured. Set DEEPSEEK_API_KEY, OPENAI_API_KEY, or OPENROUTER_API_KEY".to_string()))
    }
}
