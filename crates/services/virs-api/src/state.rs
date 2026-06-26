//! Application state shared across all API handlers.

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::FromRef;
use tokio::sync::{broadcast, mpsc};

use virs_bot::auto::types::AutoCommand;
use virs_bot::grid::types::GridCommand;
use virs_exchange::Exchanges;
use virs_market::{KlineEngine, OrderBookEngine};
use virs_types::position::EngineEvent;

/// WebSocket 广播器
pub struct WsBroadcaster {
    tx: tokio::sync::broadcast::Sender<serde_json::Value>,
}

impl WsBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(256);
        Self { tx }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<serde_json::Value> {
        self.tx.subscribe()
    }

    pub fn broadcast(&self, value: serde_json::Value) {
        let _ = self.tx.send(value);
    }
}

/// Engine manager trait — lazy initialization of trading engines.
/// Implemented in virs-app (composition root) with access to all adapters.
#[async_trait]
pub trait EngineManager: Send + Sync {
    /// Ensure all engines (Position, Grid, Auto) are started.
    /// Called when the first bot is created after wizard completion.
    /// `paper_mode` only takes effect on the first call; subsequent calls are no-ops.
    async fn ensure_started(&self, paper_mode: bool) -> Result<(), String>;

    /// Get the grid engine command sender (None if engines not started)
    fn grid_cmd_tx(&self) -> Option<mpsc::Sender<GridCommand>>;

    /// Get the auto engine command sender (None if engines not started)
    fn auto_cmd_tx(&self) -> Option<mpsc::Sender<AutoCommand>>;

    /// Whether engines have been started
    fn is_started(&self) -> bool;

    /// Current paper mode (meaningful only after engines started)
    fn paper_mode(&self) -> bool;

    /// Register a symbol for paper mode price ticks
    async fn register_paper_symbol(&self, exchange: String, symbol: String);

    /// 订阅 Position Engine 事件（用于 /ws/position 推送）
    fn pe_event_subscribe(&self) -> Option<broadcast::Receiver<EngineEvent>>;

    /// 查询 PE 中指定 symbol 的当前仓位快照（用于 /ws/position subscribe 时推送）
    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<virs_types::position::Position>;

    /// Restore services if bots exist in DB but engines are not started.
    /// Called once at server startup. No-op if engines already started or no bots exist.
    async fn restore_if_needed(&self);
}

/// API 应用状态
#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub ws_broadcaster: Arc<WsBroadcaster>,
    pub engine_manager: Arc<dyn EngineManager>,
    pub http_client: reqwest::Client,
    pub exchange_registry: Arc<Exchanges>,
    pub kline_engine: Arc<KlineEngine>,
    pub orderbook_engine: Arc<OrderBookEngine>,
    pub encryption_key: String,
}

impl FromRef<AppState> for sqlx::PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.db_pool.clone()
    }
}
