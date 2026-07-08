//! LLM Provider 统一配置
//!
//! 定义各 LLM Provider 的 base URL、默认模型名和余额查询 URL。
//! `virs-api/handlers/ai.rs` 和 `virs-app/adapters/llm_resolver.rs` 共用此模块，
//! 消除重复定义。

/// LLM Provider 配置
pub struct LlmProviderConfig {
    pub base_url: &'static str,
    pub default_model: &'static str,
    /// 余额查询 URL（仅支持余额查询的 provider，不支持则 None）
    pub balance_url: Option<&'static str>,
}

/// 已知 Provider 列表
pub const KNOWN_PROVIDERS: &[&str] = &["deepseek", "openai", "openrouter"];

/// 获取 Provider 配置
///
/// 返回 `Option<&LlmProviderConfig>`，未知 provider 返回 None。
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

/// 解析 Provider 的 base URL
pub fn resolve_provider_base_url(provider: &str) -> Option<&'static str> {
    get_provider_config(provider).map(|c| c.base_url)
}

/// 解析 Provider 的默认模型名
pub fn resolve_provider_model(provider: &str) -> Option<&'static str> {
    get_provider_config(provider).map(|c| c.default_model)
}

/// 解析 Provider 的余额查询 URL
pub fn resolve_provider_balance_url(provider: &str) -> Option<&'static str> {
    get_provider_config(provider).and_then(|c| c.balance_url)
}
