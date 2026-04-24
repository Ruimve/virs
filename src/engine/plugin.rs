use crate::models::Kline;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parameter definition for an indicator plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub name: String,
    pub label: String,
    pub param_type: ParamType,
    pub default: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    Int,
    Float,
}

/// Information about a registered plugin (returned by API).
#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub description: String,
    pub category: String,
    pub params: Vec<ParamDef>,
}

/// Indicator plugin trait.
/// Plugins receive kline data and parameters, and return a signal.
pub trait IndicatorPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn category(&self) -> &str;
    fn params(&self) -> Vec<ParamDef>;

    /// Generate a signal: 1 = buy, -1 = sell, 0 = hold.
    fn signal(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> i8;
}

/// Plugin registry that manages all available indicator plugins.
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn IndicatorPlugin>>,
    aliases: HashMap<String, String>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn register(&mut self, plugin: Box<dyn IndicatorPlugin>) {
        let name = plugin.name().to_string();
        tracing::info!("Registered indicator plugin: {}", name);
        self.plugins.insert(name, plugin);
    }

    /// Register an alias that resolves to an existing plugin name.
    pub fn register_alias(&mut self, alias: &str, target: &str) {
        tracing::info!("Registered plugin alias: {} -> {}", alias, target);
        self.aliases.insert(alias.to_string(), target.to_string());
    }

    /// Resolve a name through aliases to the canonical plugin name.
    fn resolve<'a>(&'a self, name: &'a str) -> &'a str {
        self.aliases.get(name).map(|s| s.as_str()).unwrap_or(name)
    }

    pub fn get(&self, name: &str) -> Option<&dyn IndicatorPlugin> {
        let resolved = self.resolve(name);
        self.plugins.get(resolved).map(|p| p.as_ref())
    }

    pub fn list(&self) -> Vec<PluginInfo> {
        self.plugins
            .values()
            .map(|p| PluginInfo {
                name: p.name().to_string(),
                description: p.description().to_string(),
                category: p.category().to_string(),
                params: p.params(),
            })
            .collect()
    }

    /// Generate a signal using a named plugin.
    pub fn generate_signal(
        &self,
        plugin_name: &str,
        klines: &[Kline],
        idx: usize,
        params: &HashMap<String, f64>,
    ) -> anyhow::Result<i8> {
        let plugin = self
            .get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_name))?;
        Ok(plugin.signal(klines, idx, params))
    }
}
