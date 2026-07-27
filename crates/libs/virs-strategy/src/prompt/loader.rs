//! Prompt 模板文件夹加载器。
//!
//! 启动流程：
//! 1. 读取 `STRATEGIES_DIR` 环境变量。未设置时返回空 loader（worker 回退默认常量）
//! 2. 扫描 `{dir}/auto/*/` 和 `{dir}/grid/*/` 子目录（每个子目录 = 一个策略）
//! 3. 每个子目录读取 `meta.json` + `system_prompt.md` + `user_prompt_template.md`，
//!    组装为 [`PromptTemplate`]，调用 [`validator::validate`] 校验
//! 4. 校验通过则缓存，失败则 `warn!` 记录并跳过（不中断启动）
//!
//! 运行时查询：[`PromptLoader::get`] 按 `(strategy_type, name)` 查找。
//! 不做文件 watcher 热更新——改 prompt 需重启 bot，避免运行中行为突变。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{info, warn};
use virs_error::{Context, VirsResult};

use crate::prompt::template::{MetaFile, PromptTemplate, StrategyType};
use crate::prompt::validator::validate;

/// 环境变量名。
pub const ENV_STRATEGIES_DIR: &str = "STRATEGIES_DIR";

/// Prompt 模板加载器。线程安全，可全局共享。
#[derive(Debug, Clone)]
pub struct PromptLoader {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Debug, Default)]
struct Inner {
    /// key = (strategy_type, name)，value = 模板
    templates: HashMap<(StrategyType, String), PromptTemplate>,
    /// 实际扫描的根目录（用于日志/调试）
    root_dir: Option<PathBuf>,
}

impl PromptLoader {
    /// 创建空 loader（未配置 `STRATEGIES_DIR` 时使用）。
    pub fn empty() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner::default())),
        }
    }

    /// 从 `STRATEGIES_DIR` 环境变量加载。环境变量未设置时返回空 loader。
    pub async fn from_env() -> Self {
        match std::env::var(ENV_STRATEGIES_DIR) {
            Ok(dir) if !dir.trim().is_empty() => Self::from_dir(PathBuf::from(dir)).await,
            Ok(_) => {
                warn!(
                    env = ENV_STRATEGIES_DIR,
                    "STRATEGIES_DIR is set to empty — prompt loader disabled, workers will use built-in defaults"
                );
                Self::empty()
            }
            Err(_) => {
                info!(
                    env = ENV_STRATEGIES_DIR,
                    "STRATEGIES_DIR not set — prompt loader disabled, workers will use built-in defaults"
                );
                Self::empty()
            }
        }
    }

    /// 从指定目录加载。扫描 `{dir}/auto/*/` 和 `{dir}/grid/*/` 子目录。
    pub async fn from_dir(dir: PathBuf) -> Self {
        let mut inner = Inner {
            templates: HashMap::new(),
            root_dir: Some(dir.clone()),
        };

        if !dir.exists() {
            warn!(
                dir = %dir.display(),
                "STRATEGIES_DIR points to non-existent directory — prompt loader disabled"
            );
            return Self {
                inner: Arc::new(RwLock::new(inner)),
            };
        }

        for st in [StrategyType::Auto, StrategyType::Grid] {
            let sub = dir.join(st.as_dir());
            if !sub.exists() {
                info!(subdir = %sub.display(), "strategy subdir not found, skipping");
                continue;
            }
            load_subdir(&sub, st, &mut inner).await;
        }

        info!(
            dir = %dir.display(),
            loaded = inner.templates.len(),
            "PromptLoader loaded strategy templates"
        );
        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    /// 查询模板。未配置目录或未找到时返回 `None`，由调用方决定回退策略。
    pub async fn get(&self, strategy_type: StrategyType, name: &str) -> Option<PromptTemplate> {
        self.inner
            .read()
            .await
            .templates
            .get(&(strategy_type, name.to_string()))
            .cloned()
    }

    /// 列出指定策略类型的全部模板名（用于 API/调试）。
    pub async fn list(&self, strategy_type: StrategyType) -> Vec<String> {
        self.inner
            .read()
            .await
            .templates
            .keys()
            .filter(|(st, _)| *st == strategy_type)
            .map(|(_, name)| name.clone())
            .collect()
    }

    /// 已加载的模板总数。
    pub async fn len(&self) -> usize {
        self.inner.read().await.templates.len()
    }

    /// 是否为空（未加载任何模板）。
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.templates.is_empty()
    }

    /// 返回加载的根目录（用于诊断）。
    pub async fn root_dir(&self) -> Option<PathBuf> {
        self.inner.read().await.root_dir.clone()
    }

    /// 插入或更新单个模板到内存缓存。
    ///
    /// 供 `save_template` 写入文件后调用,使后续 `get` / `list` 立即返回新内容。
    /// 若同名 key 已存在则覆盖（语义与启动时扫描一致——"last loaded wins"）。
    pub async fn upsert(&self, template: PromptTemplate) {
        let key = (template.strategy_type, template.name.clone());
        let mut guard = self.inner.write().await;
        if guard.templates.insert(key, template).is_some() {
            warn!("upsert overwrote existing strategy template");
        }
    }

    /// 从内存缓存移除单个模板。
    ///
    /// 供 `delete_template` 删除文件后调用,使后续 `get` / `list` 不再返回已删除的策略。
    /// key 不存在时静默返回（幂等）。
    pub async fn remove(&self, strategy_type: StrategyType, name: &str) {
        let key = (strategy_type, name.to_string());
        let mut guard = self.inner.write().await;
        guard.templates.remove(&key);
    }
}

async fn load_subdir(sub: &Path, st: StrategyType, inner: &mut Inner) {
    let mut entries = match tokio::fs::read_dir(sub).await {
        Ok(rd) => rd,
        Err(e) => {
            warn!(
                subdir = %sub.display(),
                error = %e,
                "Failed to read strategy subdir — skipping"
            );
            return;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        match load_strategy_folder(&path, st, &name).await {
            Ok(tpl) => {
                let key = (st, tpl.name.clone());
                if inner.templates.insert(key, tpl).is_some() {
                    warn!(
                        subdir = %sub.display(),
                        name = %name,
                        "Duplicate strategy name — last loaded wins"
                    );
                }
            }
            Err(e) => {
                warn!(
                    dir = %path.display(),
                    error = %e,
                    "Failed to load strategy folder — skipping"
                );
            }
        }
    }
}

/// 从单个策略文件夹加载模板。
///
/// 文件夹内必须包含 `meta.json` + `system_prompt.md` + `user_prompt_template.md`。
/// `name` 以文件夹名为准，`strategy_type` 以父目录为准（防止放错位置）。
async fn load_strategy_folder(
    dir: &Path,
    st: StrategyType,
    name: &str,
) -> VirsResult<PromptTemplate> {
    let meta_path = dir.join("meta.json");
    let system_path = dir.join("system_prompt.md");
    let user_path = dir.join("user_prompt_template.md");

    let meta_data = tokio::fs::read(&meta_path)
        .await
        .context("读取 meta.json 失败")?;
    let mut meta: MetaFile = serde_json::from_slice(&meta_data)
        .context("解析 meta.json 失败")?;

    let system_prompt = tokio::fs::read_to_string(&system_path)
        .await
        .context("读取 system_prompt.md 失败")?;
    let user_prompt_template = tokio::fs::read_to_string(&user_path)
        .await
        .context("读取 user_prompt_template.md 失败")?;

    // name 以文件夹名为准（防止 meta.json 内 name 与文件夹名不一致）
    meta.name = name.to_string();
    // strategy_type 以目录为准（防止放错目录）
    meta.strategy_type = st;

    let tpl = PromptTemplate {
        name: meta.name,
        strategy_type: meta.strategy_type,
        system_prompt,
        user_prompt_template,
        required_placeholders: meta.required_placeholders,
        source: meta.source,
        version: meta.version,
        description: meta.description,
        created_at: meta.created_at,
    };

    validate(&tpl).context("校验失败")?;

    Ok(tpl)
}

/// 同步加载辅助函数（用于测试或非 async 上下文）。
pub fn load_dir_blocking(dir: PathBuf) -> PromptLoader {
    let mut inner = Inner {
        templates: HashMap::new(),
        root_dir: Some(dir.clone()),
    };
    if !dir.exists() {
        return PromptLoader {
            inner: Arc::new(RwLock::new(inner)),
        };
    }
    for st in [StrategyType::Auto, StrategyType::Grid] {
        let sub = dir.join(st.as_dir());
        if !sub.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&sub) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = match path.file_name().and_then(|s| s.to_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };

                let meta_path = path.join("meta.json");
                let system_path = path.join("system_prompt.md");
                let user_path = path.join("user_prompt_template.md");

                let meta_data = match std::fs::read(&meta_path) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                let mut meta: MetaFile = match serde_json::from_slice(&meta_data) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let system_prompt = match std::fs::read_to_string(&system_path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let user_prompt_template = match std::fs::read_to_string(&user_path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                meta.name = name;
                meta.strategy_type = st;

                let tpl = PromptTemplate {
                    name: meta.name,
                    strategy_type: meta.strategy_type,
                    system_prompt,
                    user_prompt_template,
                    required_placeholders: meta.required_placeholders,
                    source: meta.source,
                    version: meta.version,
                    description: meta.description,
                    created_at: meta.created_at,
                };

                if let Err(_e) = validate(&tpl) {
                    continue;
                }
                inner.templates.insert((st, tpl.name.clone()), tpl);
            }
        }
    }
    PromptLoader {
        inner: Arc::new(RwLock::new(inner)),
    }
}
