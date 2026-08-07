/* LLM 提供者配置：封装各 LLM 服务的 base_url、默认模型和余额查询地址 */
pub struct LlmProviderConfig {
    pub base_url: &'static str,
    pub default_model: &'static str,

    /* 余额查询地址：部分提供者支持，用于检查 API 余额 */
    pub balance_url: Option<&'static str>,
}

impl LlmProviderConfig {

    /* 工厂方法：根据提供者名称返回对应的配置，不支持时返回 None */
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
