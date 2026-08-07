use crate::loader::ENV_STRATEGIES_DIR;
use crate::template::{MetaFile, PromptTemplate};
use crate::writer::{create_strategy, delete_strategy, save_new_version};
use std::sync::Mutex;
use tempfile::tempdir;
use virs_type::StrategyType;


static ENV_LOCK: Mutex<()> = Mutex::new(());

fn make_valid_template(name: &str, st: StrategyType, version: i32) -> PromptTemplate {
    PromptTemplate {
        name: name.to_string(),
        strategy_type: st,
        system_prompt: "你是引擎。返回 JSON：{...}".to_string(),
        user_prompt_template: "{h1_current_price}".to_string(),
        required_placeholders: vec!["h1_current_price".to_string()],
        version,
        description: "test strategy".to_string(),
        created_at: None,
    }
}

#[test]
fn w1_create_strategy_writes_folder() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempdir().unwrap();
    std::env::set_var(ENV_STRATEGIES_DIR, tmp.path());

    let tpl = make_valid_template("test_create", StrategyType::Auto, 1);
    let path = create_strategy(&tpl).unwrap();
    assert!(path.exists());
    assert!(path.is_dir());
    assert!(path.to_str().unwrap().ends_with("test_create"));


    assert!(path.join("meta.json").exists());


    let version_dir = path.join("v1");
    assert!(version_dir.exists());
    assert!(version_dir.is_dir());
    assert!(version_dir.join("system_prompt.md").exists());
    assert!(version_dir.join("user_prompt_template.md").exists());
    assert!(version_dir.join("required_placeholders.json").exists());
    assert!(version_dir.join("description.md").exists());


    let meta_content = std::fs::read_to_string(path.join("meta.json")).unwrap();
    let meta: MetaFile = serde_json::from_str(&meta_content).unwrap();
    assert_eq!(meta.name, "test_create");
    assert_eq!(meta.version, 1);


    let sys = std::fs::read_to_string(version_dir.join("system_prompt.md")).unwrap();
    assert_eq!(sys, tpl.system_prompt);
    let user = std::fs::read_to_string(version_dir.join("user_prompt_template.md")).unwrap();
    assert_eq!(user, tpl.user_prompt_template);

    let rp: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(version_dir.join("required_placeholders.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(rp, tpl.required_placeholders);

    let desc = std::fs::read_to_string(version_dir.join("description.md")).unwrap();
    assert_eq!(desc, tpl.description);

    std::env::remove_var(ENV_STRATEGIES_DIR);
}

#[test]
fn w2_create_strategy_rejects_existing() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempdir().unwrap();
    std::env::set_var(ENV_STRATEGIES_DIR, tmp.path());

    let tpl = make_valid_template("test_dup", StrategyType::Auto, 1);
    create_strategy(&tpl).unwrap();


    let result = create_strategy(&tpl);
    assert!(result.is_err());

    std::env::remove_var(ENV_STRATEGIES_DIR);
}

#[test]
fn w3_save_new_version_creates_v2() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempdir().unwrap();
    std::env::set_var(ENV_STRATEGIES_DIR, tmp.path());

    let tpl_v1 = make_valid_template("test_version", StrategyType::Auto, 1);
    create_strategy(&tpl_v1).unwrap();


    let mut tpl_v2 = tpl_v1.clone();
    tpl_v2.version = 2;
    tpl_v2.system_prompt = "你是改进后的引擎。返回 JSON：{...}".to_string();
    tpl_v2.description = "improved strategy".to_string();
    let path = save_new_version(&tpl_v2).unwrap();


    assert!(path.join("v1").exists());
    assert!(path.join("v2").exists());


    let v2_sys = std::fs::read_to_string(path.join("v2").join("system_prompt.md")).unwrap();
    assert_eq!(v2_sys, tpl_v2.system_prompt);

    let v2_desc = std::fs::read_to_string(path.join("v2").join("description.md")).unwrap();
    assert_eq!(v2_desc, tpl_v2.description);


    let v1_sys = std::fs::read_to_string(path.join("v1").join("system_prompt.md")).unwrap();
    assert_eq!(v1_sys, tpl_v1.system_prompt);

    let v1_desc = std::fs::read_to_string(path.join("v1").join("description.md")).unwrap();
    assert_eq!(v1_desc, tpl_v1.description);


    let meta_content = std::fs::read_to_string(path.join("meta.json")).unwrap();
    let meta: MetaFile = serde_json::from_str(&meta_content).unwrap();
    assert_eq!(meta.version, 2);

    std::env::remove_var(ENV_STRATEGIES_DIR);
}

#[test]
fn w4_save_new_version_rejects_nonexistent_strategy() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempdir().unwrap();
    std::env::set_var(ENV_STRATEGIES_DIR, tmp.path());

    let tpl = make_valid_template("nonexistent", StrategyType::Auto, 1);
    let result = save_new_version(&tpl);
    assert!(result.is_err());

    std::env::remove_var(ENV_STRATEGIES_DIR);
}

#[test]
fn w5_delete_strategy_removes_folder() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempdir().unwrap();
    std::env::set_var(ENV_STRATEGIES_DIR, tmp.path());

    let tpl = make_valid_template("test_del", StrategyType::Auto, 1);
    let path = create_strategy(&tpl).unwrap();
    assert!(path.exists());

    delete_strategy("test_del").unwrap();
    assert!(!path.exists());


    let result = delete_strategy("test_del");
    assert!(result.is_err());

    std::env::remove_var(ENV_STRATEGIES_DIR);
}

#[test]
fn w6_create_without_env_var_errors() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var(ENV_STRATEGIES_DIR);
    let tpl = make_valid_template("no_env", StrategyType::Auto, 1);
    let result = create_strategy(&tpl);
    assert!(result.is_err());
}
