use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::FromRef;
use tokio::sync::{broadcast, mpsc};

use virs_bot::auto::types::AutoCommand;
use virs_bot::grid::types::GridCommand;
use virs_error::VirsResult;
use virs_exchange::Exchanges;
use virs_market::{KlineEngine, OrderBookEngine};
use virs_types::position::EngineEvent;


#[async_trait]
pub trait EngineManager: Send + Sync {


    async fn ensure_started(&self, paper_mode: bool) -> VirsResult<()>;


    fn grid_cmd_tx(&self) -> Option<mpsc::Sender<GridCommand>>;


    fn auto_cmd_tx(&self) -> Option<mpsc::Sender<AutoCommand>>;


    fn paper_mode(&self) -> Option<bool>;


    fn restore_error(&self) -> Option<String>;


    async fn register_paper_symbol(&self, exchange: String, symbol: String);


    fn pe_event_subscribe(&self) -> Option<broadcast::Receiver<EngineEvent>>;


    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<virs_types::position::Position>;


    async fn restore_if_needed(&self) -> VirsResult<()>;


    async fn shutdown(&self);
}


#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub engine_manager: Arc<dyn EngineManager>,
    pub http_client: reqwest::Client,
    pub exchange_registry: Arc<Exchanges>,
    pub kline_engine: Arc<KlineEngine>,
    pub orderbook_engine: Arc<OrderBookEngine>,
    pub encryption_key: String,
    pub llm_key: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,


    pub http_timeout_secs: u64,

    pub http_connect_timeout_secs: u64,

    pub http_pool_max_idle_per_host: usize,

    pub listenkey_keepalive_futures_secs: u64,
}

impl FromRef<AppState> for sqlx::PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.db_pool.clone()
    }
}
