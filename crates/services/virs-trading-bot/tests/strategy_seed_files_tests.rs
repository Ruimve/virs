

use virs_prompt::PromptLoader;
use virs_type::StrategyType;

#[tokio::test]
async fn seed_auto_default_loads_and_validates() {
    let dir = std::env::var("STRATEGIES_DIR")
        .unwrap_or_else(|_| "../../strategies".to_string());
    let path = std::path::PathBuf::from(&dir);
    if !path.exists() {
        eprintln!("STRATEGIES_DIR={dir} not found, skipping seed test");
        return;
    }
    let loader = PromptLoader::from_dir(path).await;
    assert!(!loader.list(StrategyType::Chat).await.is_empty(), "loader should have loaded seed files");

    let tpl = loader
        .get(StrategyType::Chat, "default")
        .await
        .expect("strategies/default/ should load");
    assert_eq!(tpl.strategy_type, StrategyType::Chat);
    assert!(!tpl.system_prompt.is_empty());
    assert!(!tpl.user_prompt_template.is_empty());
    assert!(!tpl.required_placeholders.is_empty());
}

#[tokio::test]
async fn seed_auto_range_reversion_loads_and_validates() {
    let dir = std::env::var("STRATEGIES_DIR")
        .unwrap_or_else(|_| "../../strategies".to_string());
    let path = std::path::PathBuf::from(&dir);
    if !path.exists() {
        eprintln!("STRATEGIES_DIR={dir} not found, skipping seed test");
        return;
    }
    let loader = PromptLoader::from_dir(path).await;
    assert!(!loader.list(StrategyType::Chat).await.is_empty(), "loader should have loaded seed files");

    let tpl = loader
        .get(StrategyType::Chat, "range_reversion")
        .await
        .expect("strategies/range_reversion/ should load");
    assert_eq!(tpl.strategy_type, StrategyType::Chat);
    assert!(!tpl.system_prompt.is_empty());
    assert!(!tpl.user_prompt_template.is_empty());
    assert!(!tpl.required_placeholders.is_empty());
}
