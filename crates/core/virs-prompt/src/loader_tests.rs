use crate::loader::PromptLoader;
use virs_type::StrategyType;
use std::path::PathBuf;

#[tokio::test]
async fn l1_loads_chat_strategies() {
    let dir = std::env::var("STRATEGIES_DIR")
        .unwrap_or_else(|_| "../../strategies".to_string());
    let path = PathBuf::from(&dir);
    if !path.exists() {
        eprintln!("STRATEGIES_DIR={dir} not found, skipping");
        return;
    }
    let loader = PromptLoader::from_dir(path).await;
    assert!(!loader.list(StrategyType::Chat).await.is_empty(), "should load chat strategies");
}

#[tokio::test]
async fn l2_get_loaded_strategy() {
    let dir = std::env::var("STRATEGIES_DIR")
        .unwrap_or_else(|_| "../../strategies".to_string());
    let path = PathBuf::from(&dir);
    if !path.exists() {
        eprintln!("STRATEGIES_DIR={dir} not found, skipping");
        return;
    }
    let loader = PromptLoader::from_dir(path).await;
    let tpl = loader.get(StrategyType::Chat, "default").await;
    assert!(tpl.is_some(), "should find 'default' chat strategy");
}

#[tokio::test]
async fn l3_nonexistent_dir_returns_empty() {
    let loader = PromptLoader::from_dir(PathBuf::from("/nonexistent/strategies")).await;
    assert!(loader.list(StrategyType::Chat).await.is_empty());
}
