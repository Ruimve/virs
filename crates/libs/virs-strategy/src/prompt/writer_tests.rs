use crate::prompt::loader::ENV_STRATEGIES_DIR;
use crate::prompt::template::{MetaFile, PromptSource, PromptTemplate, StrategyType};
use crate::prompt::writer::{delete_template, save_template};
use std::sync::Mutex;
use tempfile::tempdir;

/// 所有 writer 测试都依赖全局 `STRATEGIES_DIR` 环境变量，
/// 并行运行会互相污染 —— 用此 Mutex 串行化。
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn make_valid_template(name: &str, st: StrategyType) -> PromptTemplate {
    PromptTemplate {
        name: name.to_string(),
        strategy_type: st,
        system_prompt: "你是引擎。返回 JSON：{...}".to_string(),
        user_prompt_template: "{h1_current_price}".to_string(),
        required_placeholders: vec!["h1_current_price".to_string()],
        source: PromptSource::Human,
        version: 1,
        description: "test".to_string(),
        created_at: None,
    }
}

#[test]
fn w1_save_template_writes_folder() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempdir().unwrap();
    std::env::set_var(ENV_STRATEGIES_DIR, tmp.path());

    let tpl = make_valid_template("test_save", StrategyType::Auto);
    let path = save_template(&tpl, false).unwrap();
    assert!(path.exists());
    assert!(path.is_dir());
    assert!(path.to_str().unwrap().ends_with("auto/test_save"));

    // 验证三个文件都存在
    assert!(path.join("meta.json").exists());
    assert!(path.join("system_prompt.md").exists());
    assert!(path.join("user_prompt_template.md").exists());

    // 验证 meta.json 可解析
    let meta_content = std::fs::read_to_string(path.join("meta.json")).unwrap();
    let meta: MetaFile = serde_json::from_str(&meta_content).unwrap();
    assert_eq!(meta.name, "test_save");

    // 验证 .md 文件内容
    let sys = std::fs::read_to_string(path.join("system_prompt.md")).unwrap();
    assert_eq!(sys, tpl.system_prompt);
    let user = std::fs::read_to_string(path.join("user_prompt_template.md")).unwrap();
    assert_eq!(user, tpl.user_prompt_template);

    std::env::remove_var(ENV_STRATEGIES_DIR);
}

#[test]
fn w2_save_without_overwrite_rejects_existing() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempdir().unwrap();
    std::env::set_var(ENV_STRATEGIES_DIR, tmp.path());

    let tpl = make_valid_template("test_dup", StrategyType::Auto);
    save_template(&tpl, false).unwrap();

    // 再次保存，不覆盖 → 应该报错
    let result = save_template(&tpl, false);
    assert!(result.is_err());

    // 覆盖模式 → 应该成功
    let result = save_template(&tpl, true);
    assert!(result.is_ok());

    std::env::remove_var(ENV_STRATEGIES_DIR);
}

#[test]
fn w3_delete_template_removes_folder() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempdir().unwrap();
    std::env::set_var(ENV_STRATEGIES_DIR, tmp.path());

    let tpl = make_valid_template("test_del", StrategyType::Auto);
    let path = save_template(&tpl, false).unwrap();
    assert!(path.exists());

    delete_template(StrategyType::Auto, "test_del").unwrap();
    assert!(!path.exists());

    // 再删 → not_found
    let result = delete_template(StrategyType::Auto, "test_del");
    assert!(result.is_err());

    std::env::remove_var(ENV_STRATEGIES_DIR);
}

#[test]
fn w4_save_without_env_var_errors() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var(ENV_STRATEGIES_DIR);
    let tpl = make_valid_template("no_env", StrategyType::Auto);
    let result = save_template(&tpl, false);
    assert!(result.is_err());
}
