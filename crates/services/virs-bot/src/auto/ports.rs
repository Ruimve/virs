//! Auto trading bot port definitions.

use async_trait::async_trait;
use uuid::Uuid;

pub use virs_types::bot::*;
use crate::auto::types::AutoBotConfig;

/// AI 分析日志持久化记录
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoAnalysisLogEntry {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub analysis_type: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub result: serde_json::Value,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 市场快照
#[derive(Debug, Clone, Default)]
pub struct MarketSnapshot {
    pub current_price: f64,
    pub funding_rate: f64,
    pub funding_next_time: String,
    pub indicators: crate::common::indicators::MarketIndicators,
    pub min_qty: f64,
    pub liquidation_price: Option<f64>,
}

/// 价格提供者端口
#[async_trait]
pub trait PriceProvider: Send + Sync {
    async fn get_price(&self, exchange: &str, symbol: &str, market_type: &str) -> Option<f64>;
}

/// 自动交易数据存储端口
#[async_trait]
pub trait AutoStore: Send + Sync {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<AutoBotConfig>>;
    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<AutoBotConfig>>;
    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> anyhow::Result<()>;
    async fn update_last_decided(&self, bot_id: Uuid) -> anyhow::Result<()>;
    async fn update_position(
        &self, bot_id: Uuid, current_side: Option<&str>, entry_price: f64,
        position_size: f64, stop_loss: f64, take_profit: f64,
        liquidation_price: Option<f64>,
    ) -> anyhow::Result<()>;
    async fn update_ai_analysis(
        &self, bot_id: Uuid, market_regime: &str, leverage: i32, ai_analysis: &str,
    ) -> anyhow::Result<()>;
    async fn update_stats(
        &self, bot_id: Uuid, total_pnl: f64, total_trades: i32,
        win_trades: i32, loss_trades: i32,
    ) -> anyhow::Result<()>;
    async fn record_trade(
        &self, bot_id: Uuid, user_id: Uuid, symbol: &str, exchange: &str,
        side: &str, trade_type: &str, trigger_source: &str, price: f64,
        quantity: f64, pnl: f64, pnl_pct: f64, exchange_order_id: Option<&str>,
    ) -> anyhow::Result<Uuid>;
    async fn save_analysis_log(
        &self, bot_id: Uuid, analysis_type: &str, system_prompt: &str,
        user_prompt: &str, result: &serde_json::Value, error: Option<&str>,
    ) -> anyhow::Result<()>;
    async fn load_analysis_logs(&self, bot_id: Uuid) -> anyhow::Result<Vec<AutoAnalysisLogEntry>>;
    async fn load_consecutive_losses(&self, bot_id: Uuid) -> anyhow::Result<i32>;
    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()>;
}

/// 市场数据提供者端口
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn get_market_snapshot(&self, exchange: &str, symbol: &str, market_type: &str) -> MarketSnapshot;
    async fn get_account_balance(&self, exchange: &str, market_type: &str) -> AccountBalance;
}
