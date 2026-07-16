use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use virs_error::VirsResult;


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

    /// 记录开仓 context — INSERT pe_auto_order_context (order_role='open', status='open')
    async fn record_open_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        client_order_id: &str,
        stop_loss: f64,
        take_profit: f64,
    ) -> VirsResult<()>;

    /// 记录平仓 context — UPDATE open row status='closed' + INSERT close row
    async fn close_trade(
        &self,
        open_client_order_id: &str,
        close_client_order_id: &str,
        close_reason: &str,
    ) -> VirsResult<()>;

    /// 更新止损 — UPDATE pe_auto_order_context SET stop_loss WHERE client_order_id
    async fn update_trade_stop_loss(&self, client_order_id: &str, stop_loss: f64) -> VirsResult<()>;

    /// 查找 open 状态的开仓 context — 返回 (client_order_id, stop_loss, take_profit, opened_at)
    async fn find_open_trade(
        &self,
        bot_id: Uuid,
    ) -> VirsResult<Option<(String, f64, f64, DateTime<Utc>)>>;

    /// 标记孤儿 — UPDATE status='orphaned' WHERE client_order_id
    async fn mark_trade_orphaned(&self, client_order_id: &str) -> VirsResult<()>;

    /// 查找最近已平仓交易 — 返回 (open_side, close_reason, closed_at)
    /// open_side 从 pe_orders.side 派生, close_reason 从 context 取
    async fn find_last_closed_trade(
        &self,
        bot_id: Uuid,
    ) -> VirsResult<Option<(String, String, DateTime<Utc>)>>;

    /// 记录孤儿平仓 — INSERT close context row, status='orphaned'
    async fn record_orphaned_close_trade(
        &self,
        bot_id: Uuid,
        user_id: Uuid,
        symbol: &str,
        exchange: &str,
        close_client_order_id: &str,
        close_reason: &str,
    ) -> VirsResult<()>;

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

    async fn update_analysis_log_execution(
        &self,
        log_id: Uuid,
        execution_status: &str,
        intercept_reason: Option<&str>,
    ) -> VirsResult<()>;

    /// 加载连续亏损次数 — JOIN pe_orders 取 close 订单的 realized_pnl
    async fn load_consecutive_losses(&self, bot_id: Uuid) -> VirsResult<i32>;
    async fn delete_bot(&self, bot_id: Uuid) -> VirsResult<()>;
}


#[derive(Debug, Clone)]
pub struct AutoBotConfig {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub symbol: String,
    pub exchange: String,
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
