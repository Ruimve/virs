//! Prompt 模板文件加载器。
//!
//! 启动流程：
//! 1. 读取 `STRATEGIES_DIR` 环境变量。未设置时返回空 loader（worker 回退默认常量）
//! 2. 扫描 `{dir}/auto/*.json` 和 `{dir}/grid/*.json`
//! 3. 逐文件反序列化为 [`PromptTemplate`]，调用 [`validator::validate`] 校验
//! 4. 校验通过则缓存，失败则 `warn!` 记录并跳过（不中断启动）
//!
//! 运行时查询：[`PromptLoader::get`] 按 `(strategy_type, name)` 查找。
//! 不做文件 watcher 热更新——改 prompt 需重启 bot，避免运行中行为突变。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::strategy::prompt::template::{PromptTemplate, StrategyType};
use crate::strategy::prompt::validator::validate;

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

    /// 从指定目录加载。扫描 `{dir}/auto/*.json` 和 `{dir}/grid/*.json`。
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
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        let data = match tokio::fs::read(&path).await {
            Ok(d) => d,
            Err(e) => {
                warn!(
                    file = %path.display(),
                    error = %e,
                    "Failed to read strategy file — skipping"
                );
                continue;
            }
        };

        let mut tpl: PromptTemplate = match serde_json::from_slice(&data) {
            Ok(t) => t,
            Err(e) => {
                warn!(
                    file = %path.display(),
                    error = %e,
                    "Failed to parse strategy JSON — skipping"
                );
                continue;
            }
        };

        // name 字段以文件名为准（防止 JSON 内 name 与文件名不一致）
        tpl.name = stem.clone();
        // strategy_type 以目录为准（防止放错目录）
        tpl.strategy_type = st;

        if let Err(e) = validate(&tpl) {
            warn!(
                file = %path.display(),
                error = %e,
                "Strategy template validation failed — skipping"
            );
            continue;
        }

        let key = (st, tpl.name.clone());
        if inner.templates.insert(key, tpl).is_some() {
            warn!(
                subdir = %sub.display(),
                name = stem.as_str(),
                "Duplicate strategy name — last loaded wins"
            );
        }
    }
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
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let stem = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                let data = match std::fs::read(&path) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                let mut tpl: PromptTemplate = match serde_json::from_slice(&data) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                tpl.name = stem;
                tpl.strategy_type = st;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::prompt::template::PromptSource;

    fn write_template(dir: &Path, st: StrategyType, name: &str, system: &str, user: &str) {
        let sub = dir.join(st.as_dir());
        std::fs::create_dir_all(&sub).unwrap();
        let tpl = PromptTemplate {
            name: name.to_string(),
            strategy_type: st,
            system_prompt: system.to_string(),
            user_prompt_template: user.to_string(),
            required_placeholders: vec!["h1_current_price".to_string()],
            source: PromptSource::Human,
            version: 1,
            description: String::new(),
            created_at: None,
        };
        let json = serde_json::to_string_pretty(&tpl).unwrap();
        std::fs::write(sub.join(format!("{name}.json")), json).unwrap();
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
    fn l4_strategy_type_from_directory_not_json() {
        // 文件放在 auto/ 目录但 JSON 内写 strategy_type=Grid
        // 加载器应以目录为准
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("auto");
        std::fs::create_dir_all(&sub).unwrap();
        let tpl = PromptTemplate {
            name: "mismatch".to_string(),
            strategy_type: StrategyType::Grid, // 故意写错
            system_prompt: "返回 JSON".to_string(),
            user_prompt_template: "{h1_current_price}".to_string(),
            required_placeholders: vec!["h1_current_price".to_string()],
            source: PromptSource::Human,
            version: 1,
            description: String::new(),
            created_at: None,
        };
        std::fs::write(
            sub.join("mismatch.json"),
            serde_json::to_string_pretty(&tpl).unwrap(),
        )
        .unwrap();
        let loader = load_dir_blocking(tmp.path().to_path_buf());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let loaded = rt
            .block_on(loader.get(StrategyType::Auto, "mismatch"))
            .expect("should be found under Auto (directory wins)");
        assert_eq!(loaded.strategy_type, StrategyType::Auto);
    }
}
