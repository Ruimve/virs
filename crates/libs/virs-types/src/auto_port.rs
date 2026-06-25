//! Auto trading bot port definitions — trait and associated types.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// AI 分析日志持久化记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoAnalysisLogEntry {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub analysis_type: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub result: serde_json::Value,
    pub error: Option<String>,
    pub llm_model: String,
    pub created_at: DateTime<Utc>,
}

/// 自动交易数据存储端口
#[async_trait]
pub trait AutoStore: Send + Sync {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<crate::auto_port::AutoBotConfig>>;
    async fn load_bot(&self, bot_id: Uuid) -> anyhow::Result<Option<crate::auto_port::AutoBotConfig>>;
    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> anyhow::Result<()>;
    async fn update_last_decided(&self, bot_id: Uuid) -> anyhow::Result<()>;
    async fn update_position(
        &self, bot_id: Uuid, position_id: Option<Uuid>,
    ) -> anyhow::Result<()>;
    async fn update_ai_analysis(
        &self, bot_id: Uuid, market_regime: &str, leverage: i32, ai_analysis: &str,
    ) -> anyhow::Result<()>;
    async fn update_stats(
        &self, bot_id: Uuid, total_pnl: f64, total_trades: i32,
        win_trades: i32, loss_trades: i32,
    ) -> anyhow::Result<()>;
    /// 开仓时 INSERT 一条 status='open' 的 trade 记录，返回 trade_id
    /// stop_loss/take_profit 为本次交易的风控边界（来自 LLM 决策）
    async fn record_open_trade(
        &self, bot_id: Uuid, user_id: Uuid, symbol: &str, exchange: &str,
        open_side: &str, open_price: f64, open_quantity: f64,
        open_fee: f64, open_order_id: Option<&str>,
        stop_loss: f64, take_profit: f64,
    ) -> anyhow::Result<Uuid>;
    /// 平仓时 UPDATE 对应的 trade 记录为 status='closed'
    async fn close_trade(
        &self, trade_id: Uuid, close_side: &str, close_price: f64,
        close_quantity: f64, close_order_id: Option<&str>,
        close_fee: f64, pnl: f64, pnl_pct: f64,
        trigger_source: &str, close_reason: &str,
    ) -> anyhow::Result<()>;
    /// 更新 trade 的 stop_loss（trailing stop 调整时调用）
    async fn update_trade_stop_loss(
        &self, trade_id: Uuid, stop_loss: f64,
    ) -> anyhow::Result<()>;
    /// 查找当前未平仓的 trade 记录（重启恢复用）
    /// 返回 (trade_id, stop_loss, take_profit) — 用于恢复内存中的风控边界
    async fn find_open_trade(&self, bot_id: Uuid) -> anyhow::Result<Option<(Uuid, f64, f64)>>;

    /// 查找最近一次已平仓的 trade 记录（重启恢复冷却期用）
    /// 返回 (open_side, close_reason, closed_at) — 用于恢复内存中的 last_close_event
    async fn find_last_closed_trade(
        &self, bot_id: Uuid,
    ) -> anyhow::Result<Option<(String, String, DateTime<Utc>)>>;

    /// 孤儿平仓：找不到对应开仓记录时，直接 INSERT 一条 status='orphaned' 的记录
    async fn record_orphaned_close_trade(
        &self, bot_id: Uuid, user_id: Uuid, symbol: &str, exchange: &str,
        close_side: &str, close_price: f64, close_quantity: f64,
        close_order_id: Option<&str>, close_fee: f64,
        pnl: f64, pnl_pct: f64, trigger_source: &str, close_reason: &str,
    ) -> anyhow::Result<Uuid>;
    async fn save_analysis_log(
        &self, bot_id: Uuid, analysis_type: &str, system_prompt: &str,
        user_prompt: &str, result: &serde_json::Value, error: Option<&str>,
        llm_model: &str,
    ) -> anyhow::Result<()>;
    async fn load_analysis_logs(&self, bot_id: Uuid) -> anyhow::Result<Vec<AutoAnalysisLogEntry>>;
    async fn load_consecutive_losses(&self, bot_id: Uuid) -> anyhow::Result<i32>;
    async fn delete_bot(&self, bot_id: Uuid) -> anyhow::Result<()>;
}

/// 市场类型（auto bot 专用）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoMarketType {
    #[serde(rename = "perpetual")]
    Perpetual,
    #[serde(rename = "spot")]
    Spot,
}

impl AutoMarketType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Perpetual => "perpetual",
            Self::Spot => "spot",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "spot" => Self::Spot,
            _ => Self::Perpetual,
        }
    }
}

impl std::fmt::Display for AutoMarketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 自动交易 Bot 配置
#[derive(Debug, Clone)]
pub struct AutoBotConfig {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
    pub market_type: AutoMarketType,
    pub paper_mode: bool,
    pub leverage: i32,
    pub max_position_pct: f64,
    pub decide_interval_secs: i32,
    pub position_id: Option<Uuid>,
    pub market_regime: Option<String>,
    pub ai_analysis: Option<String>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
    pub total_pnl: f64,
    pub total_trades: i32,
    pub win_trades: i32,
    pub loss_trades: i32,
    pub last_decided_at: Option<DateTime<Utc>>,
}
