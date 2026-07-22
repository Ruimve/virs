use crate::strategy::prompt::loader::{load_dir_blocking, ENV_STRATEGIES_DIR};
use crate::strategy::prompt::template::{MetaFile, PromptSource, StrategyType};
use std::path::{Path, PathBuf};

fn write_template(dir: &Path, st: StrategyType, name: &str, system: &str, user: &str) {
    let strategy_dir = dir.join(st.as_dir()).join(name);
    std::fs::create_dir_all(&strategy_dir).unwrap();
    let meta = MetaFile {
        name: name.to_string(),
        strategy_type: st,
        required_placeholders: vec!["h1_current_price".to_string()],
        source: PromptSource::Human,
        version: 1,
        description: String::new(),
        created_at: None,
    };
    let json = serde_json::to_string_pretty(&meta).unwrap();
    std::fs::write(strategy_dir.join("meta.json"), json).unwrap();
    std::fs::write(strategy_dir.join("system_prompt.md"), system).unwrap();
    std::fs::write(strategy_dir.join("user_prompt_template.md"), user).unwrap();
}

#[test]
fn l1_load_valid_template() {
    let tmp = tempfile::tempdir().unwrap();
    write_template(
        tmp.path(),
        StrategyType::Auto,
        "trend",
        "你是引擎。返回 JSON。",
        "{h1_current_price}",
    );
    let loader = load_dir_blocking(tmp.path().to_path_buf());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tpl = rt
        .block_on(loader.get(StrategyType::Auto, "trend"))
        .expect("template should be loaded");
    assert_eq!(tpl.name, "trend");
    assert_eq!(tpl.strategy_type, StrategyType::Auto);
}

#[test]
fn l2_invalid_template_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    // system_prompt 不含 JSON 关键字 → 校验失败 → 跳过
    write_template(
        tmp.path(),
        StrategyType::Auto,
        "bad",
        "you are an engine. reply in plain text.",
        "{h1_current_price}",
    );
    let loader = load_dir_blocking(tmp.path().to_path_buf());
    let rt = tokio::runtime::Runtime::new().unwrap();
    assert!(rt.block_on(loader.get(StrategyType::Auto, "bad")).is_none());
    assert!(rt.block_on(loader.is_empty()));
}

#[test]
fn l3_nonexistent_dir_returns_empty() {
    let loader = load_dir_blocking(PathBuf::from("/nonexistent/strategies"));
    let rt = tokio::runtime::Runtime::new().unwrap();
    assert!(rt.block_on(loader.is_empty()));
}

#[test]
fn l4_strategy_type_from_directory_not_meta() {
    // 文件夹放在 auto/ 目录但 meta.json 内写 strategy_type=Grid
    // 加载器应以目录为准
    let tmp = tempfile::tempdir().unwrap();
    let strategy_dir = tmp.path().join("auto").join("mismatch");
    std::fs::create_dir_all(&strategy_dir).unwrap();
    let meta = MetaFile {
        name: "mismatch".to_string(),
        strategy_type: StrategyType::Grid, // 故意写错
        required_placeholders: vec!["h1_current_price".to_string()],
        source: PromptSource::Human,
        version: 1,
        description: String::new(),
        created_at: None,
    };
    std::fs::write(
        strategy_dir.join("meta.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();
    std::fs::write(strategy_dir.join("system_prompt.md"), "返回 JSON").unwrap();
    std::fs::write(strategy_dir.join("user_prompt_template.md"), "{h1_current_price}").unwrap();
    let loader = load_dir_blocking(tmp.path().to_path_buf());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let loaded = rt
        .block_on(loader.get(StrategyType::Auto, "mismatch"))
        .expect("should be found under Auto (directory wins)");
    assert_eq!(loaded.strategy_type, StrategyType::Auto);
}
