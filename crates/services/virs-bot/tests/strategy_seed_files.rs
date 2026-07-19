//! 集成测试：验证项目根 `strategies/` 目录下的 seed 文件夹能被
//! PromptLoader 正确加载并通过校验。
//!
//! 该测试需要 `STRATEGIES_DIR` 环境变量指向项目根的 `strategies/` 目录，
//! 否则跳过（CI 中应设置该变量）。

use virs_bot::strategy::prompt::{PromptLoader, StrategyType};

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
    assert!(!loader.is_empty().await, "loader should have loaded seed files");

    let tpl = loader
        .get(StrategyType::Auto, "default")
        .await
        .expect("strategies/auto/default/ should load");
    assert_eq!(tpl.strategy_type, StrategyType::Auto);
    assert!(!tpl.system_prompt.is_empty());
    assert!(!tpl.user_prompt_template.is_empty());
    assert!(!tpl.required_placeholders.is_empty());
}

#[tokio::test]
async fn seed_grid_default_loads_and_validates() {
    let dir = std::env::var("STRATEGIES_DIR")
        .unwrap_or_else(|_| "../../strategies".to_string());
    let path = std::path::PathBuf::from(&dir);
    if !path.exists() {
        eprintln!("STRATEGIES_DIR={dir} not found, skipping seed test");
        return;
    }
    let loader = PromptLoader::from_dir(path).await;

    let tpl = loader
        .get(StrategyType::Grid, "default")
        .await
        .expect("strategies/grid/default/ should load");
    assert_eq!(tpl.strategy_type, StrategyType::Grid);
    assert!(!tpl.system_prompt.is_empty());
    assert!(!tpl.user_prompt_template.is_empty());
}
