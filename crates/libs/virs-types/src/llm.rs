pub struct LlmProviderConfig {
    pub base_url: &'static str,
    pub default_model: &'static str,

    pub balance_url: Option<&'static str>,
}


pub fn get_provider_config(provider: &str) -> Option<LlmProviderConfig> {
    match provider {
        "deepseek" => Some(LlmProviderConfig {
            base_url: "https://api.deepseek.com",
            default_model: "deepseek-chat",
            balance_url: Some("https://api.deepseek.com/user/balance"),
        }),
        "openai" => Some(LlmProviderConfig {
            base_url: "https://api.openai.com/v1",
            default_model: "gpt-4o",
            balance_url: None,
        }),
        "openrouter" => Some(LlmProviderConfig {
            base_url: "https://openrouter.ai/api/v1",
            default_model: "deepseek/deepseek-chat",
            balance_url: None,
        }),
        _ => None,
    }
}


pub fn resolve_provider_base_url(provider: &str) -> Option<&'static str> {
    get_provider_config(provider).map(|c| c.base_url)
}


pub fn resolve_provider_model(provider: &str) -> Option<&'static str> {
    get_provider_config(provider).map(|c| c.default_model)
}


pub fn resolve_provider_balance_url(provider: &str) -> Option<&'static str> {
    get_provider_config(provider).and_then(|c| c.balance_url)
}
