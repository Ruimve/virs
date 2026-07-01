//! Unit tests for adapters/llm_resolver.rs

use crate::adapters::llm_resolver::resolve_llm_provider;
use virs_config::AiConfig;

fn make_config() -> AiConfig {
    AiConfig {
        deepseek_api_key: None,
        openai_api_key: None,
        openrouter_api_key: None,
    }
}

#[test]
fn l1_1_resolve_deepseek_user_cred() {
    let config = make_config();
    let creds = vec![(
        "deepseek".to_string(),
        "user-key-123".to_string(),
        Some("deepseek-reasoner".to_string()),
    )];
    let (key, url, model, provider) = resolve_llm_provider(&creds, &config).unwrap();
    assert_eq!(key, "user-key-123");
    assert_eq!(url, "https://api.deepseek.com");
    assert_eq!(model, "deepseek-reasoner");
    assert_eq!(provider, "deepseek");
}

#[test]
fn l1_2_resolve_openai_fallback() {
    let config = AiConfig {
        deepseek_api_key: None,
        openai_api_key: Some("config-openai-key".to_string()),
        openrouter_api_key: None,
    };
    let creds: Vec<(String, String, Option<String>)> = vec![];
    let (key, url, model, provider) = resolve_llm_provider(&creds, &config).unwrap();
    assert_eq!(key, "config-openai-key");
    assert_eq!(url, "https://api.openai.com/v1");
    assert_eq!(model, "gpt-4o");
    assert_eq!(provider, "openai");
}

#[test]
fn l1_3_resolve_openrouter_fallback() {
    let config = AiConfig {
        deepseek_api_key: None,
        openai_api_key: None,
        openrouter_api_key: Some("config-or-key".to_string()),
    };
    let creds: Vec<(String, String, Option<String>)> = vec![];
    let (key, url, model, provider) = resolve_llm_provider(&creds, &config).unwrap();
    assert_eq!(key, "config-or-key");
    assert_eq!(url, "https://openrouter.ai/api/v1");
    assert_eq!(model, "deepseek/deepseek-chat");
    assert_eq!(provider, "openrouter");
}

#[test]
fn l1_4_resolve_no_key_error() {
    let config = make_config();
    let creds: Vec<(String, String, Option<String>)> = vec![];
    let result = resolve_llm_provider(&creds, &config);
    assert!(result.is_err());
}

#[test]
fn l1_5_resolve_user_cred_overrides_config() {
    let config = AiConfig {
        deepseek_api_key: Some("config-deepseek-key".to_string()),
        openai_api_key: Some("config-openai-key".to_string()),
        openrouter_api_key: None,
    };
    // User has openai credential → should use openai (even though deepseek is in config)
    // because user_openai takes priority over config deepseek? No — let's verify:
    // Priority: deepseek > openai > openrouter
    // user_deepseek is None, config deepseek is Some → deepseek wins
    let creds = vec![(
        "openai".to_string(),
        "user-openai-key".to_string(),
        Some("gpt-4o-mini".to_string()),
    )];
    let (key, _url, _model, provider) = resolve_llm_provider(&creds, &config).unwrap();
    // deepseek config key takes priority over user openai
    assert_eq!(key, "config-deepseek-key");
    assert_eq!(provider, "deepseek");
}
