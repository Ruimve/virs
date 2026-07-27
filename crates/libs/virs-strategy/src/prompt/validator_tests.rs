use crate::prompt::template::{PromptSource, PromptTemplate, StrategyType};
use crate::prompt::validator::{extract_placeholders, is_ai_generated, validate, ValidationError};
use std::collections::HashSet;

fn make_valid_template() -> PromptTemplate {
    PromptTemplate {
        name: "test".to_string(),
        strategy_type: StrategyType::Auto,
        system_prompt: "你是引擎。返回 JSON：{...}".to_string(),
        user_prompt_template: "价格：{h1_current_price}".to_string(),
        required_placeholders: vec!["h1_current_price".to_string()],
        source: PromptSource::Human,
        version: 1,
        description: String::new(),
        created_at: None,
    }
}

#[test]
fn v1_valid_template_passes() {
    assert!(validate(&make_valid_template()).is_ok());
}

#[test]
fn v2_empty_system_prompt_rejected() {
    let mut t = make_valid_template();
    t.system_prompt = "   ".to_string();
    assert!(matches!(
        validate(&t),
        Err(ValidationError::EmptySystemPrompt)
    ));
}

#[test]
fn v3_system_prompt_without_json_rejected() {
    let mut t = make_valid_template();
    t.system_prompt = "你是引擎。".to_string();
    assert!(matches!(
        validate(&t),
        Err(ValidationError::SystemPromptMissingJsonSchema)
    ));
}

#[test]
fn v4_unknown_placeholder_rejected() {
    let mut t = make_valid_template();
    t.user_prompt_template = "{unknown_field}".to_string();
    t.required_placeholders = vec!["unknown_field".to_string()];
    assert!(matches!(
        validate(&t),
        Err(ValidationError::UnknownPlaceholder(_))
    ));
}

#[test]
fn v5_declared_but_unused_rejected() {
    let mut t = make_valid_template();
    t.required_placeholders.push("h1_rsi".to_string());
    assert!(matches!(
        validate(&t),
        Err(ValidationError::DeclaredButUnused(_))
    ));
}

#[test]
fn v6_used_but_not_declared_rejected() {
    let mut t = make_valid_template();
    t.user_prompt_template = "{h1_current_price} {h1_rsi}".to_string();
    // required_placeholders 只声明 h1_current_price
    assert!(matches!(
        validate(&t),
        Err(ValidationError::UsedButNotDeclared(_))
    ));
}

#[test]
fn v7_invalid_name_rejected() {
    let mut t = make_valid_template();
    t.name = "test name!".to_string();
    assert!(matches!(validate(&t), Err(ValidationError::InvalidName)));
}

#[test]
fn v8_escape_braces_not_extracted() {
    let ph: HashSet<String> = extract_placeholders("{{not_a_placeholder}} {real}");
    assert_eq!(ph.len(), 1);
    assert!(ph.contains("real"));
}

#[test]
fn v9_ai_generated_detection() {
    let mut t = make_valid_template();
    assert!(!is_ai_generated(&t));
    t.source = PromptSource::AiGenerated {
        model: "deepseek".to_string(),
        generation_prompt: "生成趋势策略".to_string(),
    };
    assert!(is_ai_generated(&t));
}
