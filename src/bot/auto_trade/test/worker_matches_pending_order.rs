/**
 * 测试 AutoWorker::matches_pending_order 订单匹配逻辑
 * - client_order_id 包含 bot_id → true
 * - client_order_id 不包含 bot_id → false
 * - client_order_id 为 None → false
 */
use crate::bot::auto_trade::types::{AutoBotConfig, MarketType};
use crate::bot::auto_trade::worker::AutoWorker;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

fn make_bot_config() -> AutoBotConfig {
    AutoBotConfig {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        name: "test".to_string(),
        symbol: "BTC/USDT".to_string(),
        exchange: "binance".to_string(),
        market_type: MarketType::Perpetual,
        leverage: 3,
        max_position_pct: 80.0,
        decide_interval_secs: 300,
        current_side: None,
        entry_price: 0.0,
        position_size: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        unrealized_pnl: 0.0,
        market_regime: None,
        ai_analysis: None,
        system_prompt: None,
        user_prompt: None,
        total_pnl: 0.0,
        total_trades: 0,
        win_trades: 0,
        loss_trades: 0,
        last_decided_at: None,
    }
}

struct MockPriceProvider;
struct MockOrderExecutor;
struct MockAutoStore;
struct MockMarketDataProvider;
struct NoopCredentialStore;

#[async_trait::async_trait]
impl crate::bot::auto_trade::ports::PriceProvider for MockPriceProvider {
    async fn get_price(&self, _exchange: &str, _symbol: &str, _market_type: &str) -> Option<f64> { None }
}
#[async_trait::async_trait]
impl crate::trading::ports::OrderExecutor for MockOrderExecutor {
    async fn send_command(&self, _cmd: crate::trading::ports::OrderCommand) -> anyhow::Result<()> { Ok(()) }
}
#[async_trait::async_trait]
impl crate::bot::auto_trade::ports::AutoStore for MockAutoStore {
    async fn load_running_bots(&self) -> anyhow::Result<Vec<AutoBotConfig>> { Ok(vec![]) }
    async fn load_bot(&self, _bot_id: Uuid) -> anyhow::Result<Option<AutoBotConfig>> { Ok(None) }
    async fn update_bot_status(&self, _bot_id: Uuid, _status: &str) -> anyhow::Result<()> { Ok(()) }
    async fn update_last_decided(&self, _bot_id: Uuid) -> anyhow::Result<()> { Ok(()) }
    async fn update_position(&self, _bot_id: Uuid, _current_side: Option<&str>, _entry_price: f64, _position_size: f64, _stop_loss: f64, _take_profit: f64, _unrealized_pnl: f64) -> anyhow::Result<()> { Ok(()) }
    async fn update_ai_analysis(&self, _bot_id: Uuid, _market_regime: &str, _leverage: i32, _ai_analysis: &str) -> anyhow::Result<()> { Ok(()) }
    async fn update_stats(&self, _bot_id: Uuid, _total_pnl: f64, _total_trades: i32, _win_trades: i32, _loss_trades: i32) -> anyhow::Result<()> { Ok(()) }
    async fn record_trade(&self, _bot_id: Uuid, _user_id: Uuid, _symbol: &str, _exchange: &str, _side: &str, _trade_type: &str, _price: f64, _quantity: f64, _pnl: f64, _pnl_pct: f64, _exchange_order_id: Option<&str>) -> anyhow::Result<Uuid> { Ok(Uuid::new_v4()) }
    async fn save_analysis_log(&self, _bot_id: Uuid, _analysis_type: &str, _system_prompt: &str, _user_prompt: &str, _result: &serde_json::Value, _error: Option<&str>) -> anyhow::Result<()> { Ok(()) }
    async fn load_analysis_logs(&self, _bot_id: Uuid) -> anyhow::Result<Vec<crate::bot::auto_trade::ports::AutoAnalysisLogEntry>> { Ok(vec![]) }
    async fn load_consecutive_losses(&self, _bot_id: Uuid) -> anyhow::Result<i32> { Ok(0) }
    async fn delete_bot(&self, _bot_id: Uuid) -> anyhow::Result<()> { Ok(()) }
}
#[async_trait::async_trait]
impl crate::bot::auto_trade::ports::MarketDataProvider for MockMarketDataProvider {
    async fn get_market_snapshot(&self, _exchange: &str, _symbol: &str, _market_type: &str) -> crate::bot::auto_trade::ports::MarketSnapshot { Default::default() }
    async fn get_account_balance(&self, _exchange: &str, _market_type: &str) -> crate::bot::auto_trade::ports::AccountBalance { Default::default() }
}
#[async_trait::async_trait]
impl crate::bot::auto_trade::ports::CredentialStore for NoopCredentialStore {
    async fn load_credentials(&self, _user_id: Uuid) -> anyhow::Result<Vec<(String, String)>> { Ok(vec![]) }
}

fn make_worker(config: AutoBotConfig) -> AutoWorker {
    let (event_tx, event_rx) = broadcast::channel(16);
    let (auto_event_tx, _) = broadcast::channel(16);
    let ai_service = crate::bot::auto_trade::ai::AutoAiService::new(
        Box::new(crate::bot::auto_trade::engine::NoopLlmResolver),
        Box::new(NoopCredentialStore),
    );
    AutoWorker::new(
        config,
        Arc::new(MockPriceProvider),
        Arc::new(MockOrderExecutor),
        Arc::new(ai_service),
        Arc::new(MockAutoStore),
        Arc::new(MockMarketDataProvider),
        event_rx,
        auto_event_tx,
    )
}

#[test]
fn matches_when_id_contains_bot_id() {
    let config = make_bot_config();
    let worker = make_worker(config);
    let cid = format!("auto:long:{}", worker.bot.id);
    assert!(worker.matches_pending_order(Some(&cid)));
}

#[test]
fn no_match_when_id_different() {
    let config = make_bot_config();
    let worker = make_worker(config);
    let other_id = Uuid::new_v4();
    let cid = format!("auto:long:{}", other_id);
    assert!(!worker.matches_pending_order(Some(&cid)));
}

#[test]
fn no_match_when_none() {
    let config = make_bot_config();
    let worker = make_worker(config);
    assert!(!worker.matches_pending_order(None));
}

#[test]
fn matches_close_order_id() {
    let config = make_bot_config();
    let worker = make_worker(config);
    let cid = format!("auto:close:stop_loss:{}", worker.bot.id);
    assert!(worker.matches_pending_order(Some(&cid)));
}
