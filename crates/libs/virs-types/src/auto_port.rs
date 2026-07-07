//! Auto trading bot port definitions — trait and associated types.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use virs_error::VirsResult;

/// 自动交易数据存储端口
#[async_trait]
pub trait AutoStore: Send + Sync {
    async fn load_running_bots(&self) -> VirsResult<Vec<crate::auto_port::AutoBotConfig>>;
    async fn load_bot(
        &self,
        bot_id: Uuid,
    ) -> VirsResult<Option<crate::auto_port::AutoBotConfig>>;
    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> VirsResult<()>;
    async fn update_last_decided(&self, bot_id: Uuid) -> VirsResult<()>;
    async fn update_position(&self, bot_id: Uuid, position_id: Option<Uuid>) -> VirsResult<()>;
    async fn update_ai_analysis(
        &self,
        bot_id: Uuid,
        market_regime: &str,
        leverage: i32,
        ai_analysis: &str,
    ) -> VirsResult<()>;
    async fn update_stats(
        &self,
        bot_id: Uuid,
        total_pnl: f64,
        total_trades: i32,
        win_trades: i32,
        loss_trades: i32,
    ) -> VirsResult<()>;
    /// 开仓时 INSERT 一条 status='open' 的 trade 记录，返回 trade_id
    /// stop_loss/take_profit 为本次交易的风控边界（来自 LLM 决策）
    async fn record_open_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        open_side: &str,
        open_price: f64,
        open_quantity: f64,
        open_fee: f64,
        open_order_id: Option<&str>,
        stop_loss: f64,
        take_profit: f64,
    ) -> VirsResult<Uuid>;
    /// 平仓时 UPDATE 对应的 trade 记录为 status='closed'
    async fn close_trade(
        &self,
        trade_id: Uuid,
        close_side: &str,
        close_price: f64,
        close_quantity: f64,
        close_order_id: Option<&str>,
        close_fee: f64,
        pnl: f64,
        pnl_pct: f64,
        close_reason: &str,
    ) -> VirsResult<()>;
    /// 更新 trade 的 stop_loss（trailing stop 调整时调用）
    async fn update_trade_stop_loss(&self, trade_id: Uuid, stop_loss: f64) -> VirsResult<()>;
    /// 查找当前未平仓的 trade 记录（重启恢复用）
    /// 返回 (trade_id, stop_loss, take_profit, opened_at) — 用于恢复内存中的风控边界和持仓时间
    async fn find_open_trade(&self, bot_id: Uuid) -> VirsResult<Option<(Uuid, f64, f64, DateTime<Utc>)>>;

    /// 标记 trade 为孤儿状态（仅更新 status，保留开仓数据用于回溯）
    /// 用于 PE 仓位丢失但 trade 仍为 open 的场景
    async fn mark_trade_orphaned(&self, trade_id: Uuid) -> VirsResult<()>;

    /// 查找最近一次已平仓的 trade 记录（重启恢复冷却期用）
    /// 返回 (open_side, close_reason, closed_at) — 用于恢复内存中的 last_close_event
    async fn find_last_closed_trade(
        &self,
        bot_id: Uuid,
    ) -> VirsResult<Option<(String, String, DateTime<Utc>)>>;

    /// 孤儿平仓：找不到对应开仓记录时，直接 INSERT 一条 status='orphaned' 的记录
    async fn record_orphaned_close_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        close_side: &str,
        close_price: f64,
        close_quantity: f64,
        close_order_id: Option<&str>,
        close_fee: f64,
        pnl: f64,
        pnl_pct: f64,
        close_reason: &str,
    ) -> VirsResult<Uuid>;
    /// 保存 LLM 分析日志，返回日志 ID（用于后续 UPDATE 回填执行状态/拦截原因）
    async fn save_analysis_log(
        &self,
        bot_id: Uuid,
        analysis_type: &str,
        system_prompt: &str,
        user_prompt: &str,
        result: &serde_json::Value,
        error: Option<&str>,
        llm_model: &str,
    ) -> VirsResult<Uuid>;
    /// 回填 LLM 日志的执行状态、拦截原因
    /// - execution_status: open/open_failed/close/close_failed/hold
    /// - intercept_reason: 被代码拦截时的原因（如冷却期/置信度不足）
    /// 注：close_reason 不回填到此表，已记录在 qd_auto_trades.close_reason
    async fn update_analysis_log_execution(
        &self,
        log_id: Uuid,
        execution_status: &str,
        intercept_reason: Option<&str>,
    ) -> VirsResult<()>;
    async fn load_consecutive_losses(&self, bot_id: Uuid) -> VirsResult<i32>;
    async fn delete_bot(&self, bot_id: Uuid) -> VirsResult<()>;
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

    pub fn is_spot(&self) -> bool {
        self == &Self::Spot
    }

    /// Lossy parsing from a string. Unknown values default to `Perpetual`.
    /// Used by `virs-app::adapters::auto_store` when loading bot configs from DB.
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
