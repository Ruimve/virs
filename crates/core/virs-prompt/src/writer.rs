



use std::path::{Path, PathBuf};

use virs_error::{BotError, BotResult};

use crate::loader::ENV_STRATEGIES_DIR;
use crate::template::{MetaFile, PromptTemplate};
use crate::validator::validate;


/* 创建新策略：在 STRATEGIES_DIR 下创建策略文件夹和 v{version}/ 版本文件夹，写入提示词文件和 meta.json。
   策略文件夹不能已存在。meta.json 最后写入以确保原子性——崩溃时不会留下不完整的策略。 */
pub fn create_strategy(template: &PromptTemplate) -> BotResult<PathBuf> {

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

    let strategy_dir = dir.join(&template.name);
    if strategy_dir.exists() {
        return Err(BotError::Llm(format!(
            "策略文件夹已存在: {path}（如需更新请使用优化接口创建新版本）",
            path = strategy_dir.display()
        )));
    }

    let version_dir = strategy_dir.join(format!("v{}", template.version));
    std::fs::create_dir_all(&version_dir).map_err(|e| {
        BotError::Llm(format!("创建版本文件夹失败: {e}"))
    })?;

    write_version_files(&version_dir, template)?;

    let meta = MetaFile::from_template(template);
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| {
        BotError::Llm(format!("序列化 meta.json 失败: {e}"))
    })?;
    std::fs::write(strategy_dir.join("meta.json"), meta_json).map_err(|e| {
        BotError::Llm(format!("写入 meta.json 失败: {e}"))
    })?;

    tracing::info!(
        path = %strategy_dir.display(),
        name = %template.name,
        version = template.version,
        "策略已创建"
    );

    Ok(strategy_dir)
}


/* 为已有策略创建新版本：读取当前版本号并 +1，创建 v{N+1}/ 版本文件夹写入提示词文件，
   然后更新 meta.json 的 version 字段。不修改旧版本文件。meta.json 最后写入以确保原子性。 */
pub fn save_new_version(template: &PromptTemplate) -> BotResult<PathBuf> {

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
    let strategy_dir = dir.join(&template.name);
    if !strategy_dir.exists() {
        return Err(BotError::Llm(format!(
            "策略文件夹不存在: {path}（无法为新策略创建版本，请使用创建接口）",
            path = strategy_dir.display()
        )));
    }

    let version_dir = strategy_dir.join(format!("v{}", template.version));
    if version_dir.exists() {
        return Err(BotError::Llm(format!(
            "版本文件夹已存在: {path}",
            path = version_dir.display()
        )));
    }

    std::fs::create_dir_all(&version_dir).map_err(|e| {
        BotError::Llm(format!("创建版本文件夹失败: {e}"))
    })?;

    write_version_files(&version_dir, template)?;

    let meta = MetaFile::from_template(template);
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| {
        BotError::Llm(format!("序列化 meta.json 失败: {e}"))
    })?;
    std::fs::write(strategy_dir.join("meta.json"), meta_json).map_err(|e| {
        BotError::Llm(format!("写入 meta.json 失败: {e}"))
    })?;

    tracing::info!(
        path = %strategy_dir.display(),
        name = %template.name,
        version = template.version,
        "策略新版本已保存"
    );

    Ok(strategy_dir)
}


/* 删除策略文件夹（包含所有版本子文件夹） */
pub fn delete_strategy(name: &str) -> BotResult<()> {
    let dir = std::env::var(ENV_STRATEGIES_DIR).map_err(|_| {
        BotError::Llm(format!(
            "{env} 环境变量未设置",
            env = ENV_STRATEGIES_DIR
        ))
    })?;

    let strategy_dir = PathBuf::from(&dir).join(name);

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
        "策略已删除"
    );

    Ok(())
}


/* 向版本文件夹写入四个内容文件：路径从 MetaFile::from_template() 获取，确保与 meta.json 中记录的路径一致 */
fn write_version_files(version_dir: &Path, template: &PromptTemplate) -> BotResult<()> {
    let meta = MetaFile::from_template(template);

    std::fs::write(version_dir.join(MetaFile::filename(&meta.system_prompt)), &template.system_prompt).map_err(|e| {
        BotError::Llm(format!("写入 system_prompt.md 失败: {e}"))
    })?;

    std::fs::write(
        version_dir.join(MetaFile::filename(&meta.user_prompt)),
        &template.user_prompt_template,
    )
    .map_err(|e| {
        BotError::Llm(format!("写入 user_prompt_template.md 失败: {e}"))
    })?;

    let rp_json = serde_json::to_string_pretty(&template.required_placeholders).map_err(|e| {
        BotError::Llm(format!("序列化 required_placeholders.json 失败: {e}"))
    })?;
    std::fs::write(version_dir.join(MetaFile::filename(&meta.required_placeholders)), rp_json).map_err(|e| {
        BotError::Llm(format!("写入 required_placeholders.json 失败: {e}"))
    })?;

    std::fs::write(version_dir.join(MetaFile::filename(&meta.description)), &template.description).map_err(|e| {
        BotError::Llm(format!("写入 description.md 失败: {e}"))
    })?;

    Ok(())
}
