use virs_error::{BotError, BotResult};
use virs_types::bot::LlmProviderResolver;
use virs_types::llm;

pub fn resolve_llm_provider(
    user_credentials: &[(String, String, Option<String>)],
) -> BotResult<(String, String, String, String)> {
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

    for (provider, creds) in [
        ("deepseek", user_deepseek),
        ("openai", user_openai),
        ("openrouter", user_openrouter),
    ] {
        if let Some((key, model)) = creds {
            let config = llm::get_provider_config(provider)
                .ok_or_else(|| BotError::Llm(format!("Unknown provider: {}", provider)))?;
            let model = model.unwrap_or_else(|| config.default_model.to_string());
            return Ok((
                key,
                config.base_url.to_string(),
                model,
                provider.to_string(),
            ));
        }
    }

    Err(BotError::Llm(
        "No LLM API key configured. Set AI credentials via the wizard.".to_string(),
    ))
}

pub struct DefaultLlmResolver;

impl DefaultLlmResolver {
    pub fn new() -> Self {
        Self
    }
}

impl LlmProviderResolver for DefaultLlmResolver {
    fn is_available(&self) -> bool {
        false
    }

    fn resolve(
        &self,
        user_credentials: &[(String, String, Option<String>)],
    ) -> BotResult<(String, String, String, String)> {
        resolve_llm_provider(user_credentials)
    }
}
