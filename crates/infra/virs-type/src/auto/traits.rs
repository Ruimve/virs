use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use virs_error::VirsResult;

use super::structs::{AutoBotConfig, TradeRecord};


#[async_trait]
pub trait AutoStore: Send + Sync {
    async fn load_running_bots(&self) -> VirsResult<Vec<AutoBotConfig>>;
    async fn load_bot(
        &self,
        bot_id: Uuid,
    ) -> VirsResult<Option<AutoBotConfig>>;
    async fn update_bot_status(&self, bot_id: Uuid, status: &str) -> VirsResult<()>;
    async fn update_last_decided(&self, bot_id: Uuid) -> VirsResult<()>;
    async fn update_position(
        &self,
        bot_id: Uuid,
        position_id_long: Option<Uuid>,
        position_id_short: Option<Uuid>,
    ) -> VirsResult<()>;
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
        client_order_id: &str,
        stop_loss: f64,
        take_profit: f64,
        strategy_file: &Option<String>,
    ) -> VirsResult<()>;


    async fn close_trade(
        &self,
        open_client_order_id: &str,
        close_client_order_id: &str,
        close_reason: &str,
    ) -> VirsResult<()>;


    async fn update_trade_stop_loss(&self, client_order_id: &str, stop_loss: f64) -> VirsResult<()>;


    async fn find_open_trade(
        &self,
        bot_id: Uuid,
    ) -> VirsResult<Option<(String, f64, f64, DateTime<Utc>)>>;


    async fn mark_trade_orphaned(&self, client_order_id: &str) -> VirsResult<()>;


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
        close_client_order_id: &str,
        close_reason: &str,
        strategy_file: &Option<String>,
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
        strategy_file: &Option<String>,
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


#[async_trait]
pub trait TradeHistoryProvider: Send + Sync {

    async fn query_trades(
        &self,
        strategy_name: &str,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Vec<TradeRecord>;
}
