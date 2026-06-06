//! Grid bot port definitions.

use async_trait::async_trait;
use uuid::Uuid;

pub use virs_types::bot::*;

/// AI 分析日志持久化记录
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalysisLogEntry {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub analysis_type: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub result: serde_json::Value,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 网格交易记录
#[derive(Debug, Clone)]
pub struct GridTradeRecord {
    pub id: Uuid,
    pub grid_level: i32,
    pub open_side: String,
    pub open_price: f64,
    pub open_quantity: f64,
    pub close_side: Option<String>,
    pub close_price: Option<f64>,
    pub close_quantity: Option<f64>,
    pub pnl: f64,
    pub opened_at: chrono::DateTime<chrono::Utc>,
}

/// 网格 Bot 配置
#[derive(Debug, Clone)]
pub struct GridBotConfig {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub grid_count: i32,
    pub upper_price: f64,
    pub lower_price: f64,
    pub grid_profit_pct: f64,
    pub quantity_per_grid: f64,
    pub leverage: i32,
    pub dynamic_adjust: bool,
    pub adjust_interval_secs: i32,
    pub market_regime: Option<String>,
    pub grid_levels_json: Option<serde_json::Value>,
    pub system_prompt: Option<String>,
    pub last_adjusted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 价格提供者端口
#[async_trait]
pub trait PriceProvider: Send + Sync {
    async fn get_price(&self, exchange: &str, symbol: &str) -> Option<f64>;
}

/// 网格数据存储端口
#[async_trait]
pub trait GridStore: Send + Sync {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<GridBotConfig>>;
    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<GridBotConfig>>;
    async fn load_trades(&self, bot_id: Uuid) -> anyhow::Result<Vec<GridTradeRecord>>;
    async fn record_open_trade(
        &self, bot_id: Uuid, user_id: Uuid, symbol: &str, exchange: &str,
        grid_level: i32, open_side: &str, open_price: f64, open_quantity: f64,
        open_order_id: Option<&str>,
    ) -> anyhow::Result<Uuid>;
    async fn close_trade(
        &self, trade_id: Uuid, close_side: &str, close_price: f64,
        close_quantity: f64, close_order_id: Option<&str>, pnl: f64, pnl_pct: f64,
    ) -> anyhow::Result<()>;
    async fn find_open_trade(&self, bot_id: Uuid, grid_level: i32) -> anyhow::Result<Option<Uuid>>;
    async fn record_orphaned_close_trade(
        &self, bot_id: Uuid, user_id: Uuid, symbol: &str, exchange: &str,
        grid_level: i32, close_side: &str, close_price: f64, close_quantity: f64,
        close_order_id: Option<&str>, pnl: f64, pnl_pct: f64,
    ) -> anyhow::Result<Uuid>;
    async fn save_stats(
        &self, bot_id: Uuid, total_pnl: f64, unrealized_pnl: f64,
        total_trades: i32, grid_filled_count: i32, levels_json: Option<&serde_json::Value>,
    ) -> anyhow::Result<()>;
    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> anyhow::Result<()>;
    async fn update_last_adjusted(&self, bot_id: Uuid) -> anyhow::Result<()>;
    async fn update_grid_params(&self, bot_id: Uuid, upper_price: f64, lower_price: f64) -> anyhow::Result<()>;
    async fn update_quantity_per_grid(&self, bot_id: Uuid, quantity: f64) -> anyhow::Result<()>;
    async fn update_ai_analysis(
        &self, bot_id: Uuid, market_regime: &str, upper_price: f64, lower_price: f64,
        grid_count: i32, grid_profit_pct: f64, quantity_per_grid: f64, leverage: i32,
        ai_analysis: &str,
    ) -> anyhow::Result<()>;
    async fn save_analysis_log(
        &self, bot_id: Uuid, analysis_type: &str, system_prompt: &str,
        user_prompt: &str, result: &serde_json::Value, error: Option<&str>,
    ) -> anyhow::Result<()>;
    async fn load_analysis_logs(&self, bot_id: Uuid) -> anyhow::Result<Vec<AnalysisLogEntry>>;
    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()>;
}

/// 市场快照
#[derive(Debug, Clone, Default)]
pub struct MarketSnapshot {
    pub current_price: f64,
    pub indicators: crate::common::indicators::MarketIndicators,
}

/// 市场数据提供者端口
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn get_market_snapshot(&self, exchange: &str, symbol: &str) -> MarketSnapshot;
    async fn get_account_balance(&self, exchange: &str) -> AccountBalance;
}
