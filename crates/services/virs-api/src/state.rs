//! Application state shared across all API handlers.

use std::sync::Arc;

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

/// API 应用状态
#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub ws_broadcaster: Arc<WsBroadcaster>,
    pub grid_cmd_tx: Option<mpsc::Sender<GridCommand>>,
    pub auto_cmd_tx: Option<mpsc::Sender<AutoCommand>>,
    pub paper_mode: bool,
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
