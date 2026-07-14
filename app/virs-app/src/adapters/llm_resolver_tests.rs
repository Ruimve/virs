use crate::adapters::llm_resolver::resolve_llm_provider;

#[test]
fn l1_1_resolve_deepseek_user_cred() {
    let creds = vec![(
        "deepseek".to_string(),
        "user-key-123".to_string(),
        Some("deepseek-reasoner".to_string()),
    )];
    let (key, url, model, provider) = resolve_llm_provider(&creds).unwrap();
    assert_eq!(key, "user-key-123");
    assert_eq!(url, "https://api.deepseek.com");
    assert_eq!(model, "deepseek-reasoner");
    assert_eq!(provider, "deepseek");
}

#[test]
fn l1_2_resolve_openai_user_cred() {
    let creds = vec![(
        "openai".to_string(),
        "user-openai-key".to_string(),
        Some("gpt-4o-mini".to_string()),
    )];
    let (key, url, model, provider) = resolve_llm_provider(&creds).unwrap();
    assert_eq!(key, "user-openai-key");
    assert_eq!(url, "https://api.openai.com/v1");
    assert_eq!(model, "gpt-4o-mini");
    assert_eq!(provider, "openai");
}

#[test]
fn l1_3_resolve_openrouter_user_cred() {
    let creds = vec![(
        "openrouter".to_string(),
        "user-or-key".to_string(),
        None,
    )];
    let (key, url, model, provider) = resolve_llm_provider(&creds).unwrap();
    assert_eq!(key, "user-or-key");
    assert_eq!(url, "https://openrouter.ai/api/v1");
    assert_eq!(model, "deepseek/deepseek-chat");
    assert_eq!(provider, "openrouter");
}

#[test]
fn l1_4_resolve_no_key_error() {
    let creds: Vec<(String, String, Option<String>)> = vec![];
    let result = resolve_llm_provider(&creds);
    assert!(result.is_err());
}

#[test]
fn l1_5_resolve_deepseek_priority_over_openai() {

    let creds = vec![
        ("openai".to_string(), "openai-key".to_string(), None),
        ("deepseek".to_string(), "deepseek-key".to_string(), None),
    ];
    let (key, _url, _model, provider) = resolve_llm_provider(&creds).unwrap();
    assert_eq!(key, "deepseek-key");
    assert_eq!(provider, "deepseek");
}
