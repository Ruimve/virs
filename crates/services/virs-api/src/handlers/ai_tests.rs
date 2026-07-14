use crate::handlers::ai::{resolve_provider_base_url, resolve_provider_model};


#[test]
fn ai1_1_deepseek_url() {
    assert_eq!(resolve_provider_base_url("deepseek"), Some("https://api.deepseek.com"));
}

#[test]
fn ai1_2_openai_url() {
    assert_eq!(resolve_provider_base_url("openai"), Some("https://api.openai.com/v1"));
}

#[test]
fn ai1_3_openrouter_url() {
    assert_eq!(resolve_provider_base_url("openrouter"), Some("https://openrouter.ai/api/v1"));
}

#[test]
fn ai1_4_unknown_url() {
    assert_eq!(resolve_provider_base_url("unknown"), None);
}


#[test]
fn ai2_1_deepseek_model() {
    assert_eq!(resolve_provider_model("deepseek"), Some("deepseek-chat"));
}

#[test]
fn ai2_2_openai_model() {
    assert_eq!(resolve_provider_model("openai"), Some("gpt-4o"));
}

#[test]
fn ai2_3_openrouter_model() {
    assert_eq!(resolve_provider_model("openrouter"), Some("deepseek/deepseek-chat"));
}

#[test]
fn ai2_4_unknown_model() {
    assert_eq!(resolve_provider_model("unknown"), None);
}
