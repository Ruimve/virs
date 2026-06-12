//! Application state shared across all API handlers.

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::FromRef;
use tokio::sync::mpsc;

use virs_bot::grid::types::GridCommand;
use virs_bot::auto::types::AutoCommand;
use virs_exchange::ExchangeRegistry;
use virs_market::KlineEngine;

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
}

/// API 应用状态
#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub ws_broadcaster: Arc<WsBroadcaster>,
    pub engine_manager: Arc<dyn EngineManager>,
    pub http_client: reqwest::Client,
    pub exchange_registry: Arc<ExchangeRegistry>,
    pub kline_engine: Arc<KlineEngine>,
    pub encryption_key: String,
}

impl FromRef<AppState> for sqlx::PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.db_pool.clone()
    }
}
