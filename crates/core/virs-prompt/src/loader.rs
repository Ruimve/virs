

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{info, warn};
use virs_error::{Context, VirsResult};
use virs_type::StrategyType;

use crate::template::{MetaFile, PromptTemplate};
use crate::validator::validate;


pub const ENV_STRATEGIES_DIR: &str = "STRATEGIES_DIR";


#[derive(Debug, Clone)]
pub struct PromptLoader {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Debug, Default)]
struct Inner {

    templates: HashMap<(StrategyType, String), PromptTemplate>,

    root_dir: Option<PathBuf>,
}

impl PromptLoader {

    pub fn empty() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner::default())),
        }
    }


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

        let st = StrategyType::Auto;
        let sub = dir.join(st.as_dir());
        if sub.exists() {
            load_subdir(&sub, st, &mut inner).await;
        } else {
            info!(subdir = %sub.display(), "strategy subdir not found, skipping");
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


    pub async fn get(&self, strategy_type: StrategyType, name: &str) -> Option<PromptTemplate> {
        self.inner
            .read()
            .await
            .templates
            .get(&(strategy_type, name.to_string()))
            .cloned()
    }


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


    pub async fn len(&self) -> usize {
        self.inner.read().await.templates.len()
    }


    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.templates.is_empty()
    }


    pub async fn root_dir(&self) -> Option<PathBuf> {
        self.inner.read().await.root_dir.clone()
    }


    pub async fn upsert(&self, template: PromptTemplate) {
        let key = (template.strategy_type, template.name.clone());
        let mut guard = self.inner.write().await;
        if guard.templates.insert(key, template).is_some() {
            warn!("upsert overwrote existing strategy template");
        }
    }


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


    meta.name = name.to_string();

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
