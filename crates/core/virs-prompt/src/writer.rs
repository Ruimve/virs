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

use crate::loader::ENV_STRATEGIES_DIR;
use crate::template::{MetaFile, PromptTemplate};
use crate::validator::validate;

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
        strategy_type = %template.strategy_type.as_dir(),
        "策略模板已保存"
    );

    Ok(strategy_dir)
}

/// 删除策略模板文件夹。
pub fn delete_template(
    strategy_type: virs_type::StrategyType,
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
        name = %name,
        "策略模板已删除"
    );

    Ok(())
}
