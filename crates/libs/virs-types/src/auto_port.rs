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

    async fn update_trade_stop_loss(&self, trade_id: Uuid, stop_loss: f64) -> VirsResult<()>;


    async fn find_open_trade(&self, bot_id: Uuid) -> VirsResult<Option<(Uuid, f64, f64, DateTime<Utc>)>>;


    async fn mark_trade_orphaned(&self, trade_id: Uuid) -> VirsResult<()>;


    async fn find_last_closed_trade(
        &self,
        bot_id: Uuid,
    ) -> VirsResult<Option<(String, String, DateTime<Utc>)>>;


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
