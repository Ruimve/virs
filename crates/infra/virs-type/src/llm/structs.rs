pub struct LlmProviderConfig {
    pub base_url: &'static str,
    pub default_model: &'static str,

    pub balance_url: Option<&'static str>,
}

impl LlmProviderConfig {
    /// 根据 provider 名称查找静态配置（deepseek/openai/openrouter）。
    pub fn for_provider(provider: &str) -> Option<Self> {
        match provider {
            "deepseek" => Some(Self {
                base_url: "https://api.deepseek.com",
                default_model: "deepseek-chat",
                balance_url: Some("https://api.deepseek.com/user/balance"),
            }),
            "openai" => Some(Self {
                base_url: "https://api.openai.com/v1",
                default_model: "gpt-4o",
                balance_url: None,
            }),
            "openrouter" => Some(Self {
                base_url: "https://openrouter.ai/api/v1",
                default_model: "deepseek/deepseek-chat",
                balance_url: None,
            }),
            _ => None,
        }
    }
}
