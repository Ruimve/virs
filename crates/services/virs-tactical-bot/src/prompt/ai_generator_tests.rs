use crate::prompt::ai_generator::{build_meta_system_prompt, build_meta_user_prompt, GenerateRequest};
use virs_type::StrategyType;

#[test]
fn g1_meta_system_prompt_contains_json_constraint() {
    let s = build_meta_system_prompt(StrategyType::Auto);
    assert!(s.contains("JSON"));
    assert!(s.contains("open_long"));
}

#[test]
fn g3_meta_user_prompt_contains_intent() {
    let req = GenerateRequest {
        strategy_type: StrategyType::Auto,
        user_intent: "做多趋势策略",
        name_hint: Some("my_trend"),
    };
    let u = build_meta_user_prompt(&req);
    assert!(u.contains("做多趋势策略"));
    assert!(u.contains("my_trend"));
}
