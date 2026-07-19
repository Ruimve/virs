//! 策略 prompt 文件夹写入器。
//!
//! 将校验通过的 [`PromptTemplate`] 写入 `STRATEGIES_DIR/{strategy_type}/{name}/` 文件夹。
//! 文件夹内含三个文件：
//! - `meta.json` — 除 system_prompt / user_prompt_template 外的全部字段
//! - `system_prompt.md` — system_prompt 原文（Markdown，可直接编辑查看）
//! - `user_prompt_template.md` — user_prompt_template 原文
//!
//! 写入前自动校验，写入后返回文件夹路径。
//!
//! 注意：写入后运行中的 bot 不会自动热更新——需重启 bot 才能加载新 prompt。
//! 这是有意为之，避免运行中策略行为突变。

use std::path::PathBuf;

use virs_error::{BotError, BotResult};

use crate::strategy::prompt::loader::ENV_STRATEGIES_DIR;
use crate::strategy::prompt::template::{MetaFile, PromptTemplate};
use crate::strategy::prompt::validator::validate;

/// 保存策略模板到文件夹。
///
/// - `overwrite` 为 `false` 且文件夹已存在时返回错误
/// - 写入前自动校验模板
/// - 返回写入的文件夹完整路径
pub fn save_template(template: &PromptTemplate, overwrite: bool) -> BotResult<PathBuf> {
    // 先校验
    validate(template).map_err(|e| {
        BotError::Llm(format!("策略模板校验失败: {e}"))
    })?;

    let dir = std::env::var(ENV_STRATEGIES_DIR).map_err(|_| {
        BotError::Llm(format!(
            "{env} 环境变量未设置 — 无法写入策略文件",
            env = ENV_STRATEGIES_DIR
        ))
    })?;

    let dir = PathBuf::from(dir);
    if !dir.exists() {
        return Err(BotError::Llm(format!(
            "STRATEGIES_DIR 指向的目录不存在: {dir}",
            dir = dir.display()
        )));
    }

    let strategy_dir = dir
        .join(template.strategy_type.as_dir())
        .join(&template.name);

    if !overwrite && strategy_dir.exists() {
        return Err(BotError::Llm(format!(
            "策略文件夹已存在: {path}（设置 overwrite=true 可覆盖）",
            path = strategy_dir.display()
        )));
    }

    std::fs::create_dir_all(&strategy_dir).map_err(|e| {
        BotError::Llm(format!("创建策略文件夹失败: {e}"))
    })?;

    // 写入 meta.json
    let meta = MetaFile::from_template(template);
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| {
        BotError::Llm(format!("序列化 meta.json 失败: {e}"))
    })?;
    std::fs::write(strategy_dir.join("meta.json"), meta_json).map_err(|e| {
        BotError::Llm(format!("写入 meta.json 失败: {e}"))
    })?;

    // 写入 system_prompt.md
    std::fs::write(strategy_dir.join("system_prompt.md"), &template.system_prompt).map_err(|e| {
        BotError::Llm(format!("写入 system_prompt.md 失败: {e}"))
    })?;

    // 写入 user_prompt_template.md
    std::fs::write(
        strategy_dir.join("user_prompt_template.md"),
        &template.user_prompt_template,
    )
    .map_err(|e| {
        BotError::Llm(format!("写入 user_prompt_template.md 失败: {e}"))
    })?;

    tracing::info!(
        path = %strategy_dir.display(),
        name = %template.name,
        strategy_type = ?template.strategy_type,
        "策略模板已保存"
    );

    Ok(strategy_dir)
}

/// 删除策略模板文件夹。
pub fn delete_template(
    strategy_type: crate::strategy::prompt::template::StrategyType,
    name: &str,
) -> BotResult<()> {
    let dir = std::env::var(ENV_STRATEGIES_DIR).map_err(|_| {
        BotError::Llm(format!(
            "{env} 环境变量未设置",
            env = ENV_STRATEGIES_DIR
        ))
    })?;

    let strategy_dir = PathBuf::from(&dir)
        .join(strategy_type.as_dir())
        .join(name);

    if !strategy_dir.exists() {
        return Err(BotError::Llm(format!(
            "策略文件夹不存在: {path}",
            path = strategy_dir.display()
        )));
    }

    std::fs::remove_dir_all(&strategy_dir).map_err(|e| {
        BotError::Llm(format!("删除策略文件夹失败: {e}"))
    })?;

    tracing::info!(
        path = %strategy_dir.display(),
        name = name,
        "策略模板已删除"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::prompt::template::{PromptSource, StrategyType};
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

        let tpl = make_valid_template("test_dup", StrategyType::Grid);
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
}
