//! Application state shared across all API handlers.

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

/// Engine manager trait — lazy initialization of trading engines.
/// Implemented in virs-app (composition root) with access to all adapters.
#[async_trait]
pub trait EngineManager: Send + Sync {
    /// Ensure all engines (Position, Grid, Auto) are started.
    /// Called when the first bot is created after wizard completion.
    /// `paper_mode` only takes effect on the first call; subsequent calls are no-ops.
    async fn ensure_started(&self, paper_mode: bool) -> VirsResult<()>;

    /// Get the grid engine command sender (None if engines not started)
    fn grid_cmd_tx(&self) -> Option<mpsc::Sender<GridCommand>>;

    /// Get the auto engine command sender (None if engines not started)
    fn auto_cmd_tx(&self) -> Option<mpsc::Sender<AutoCommand>>;

    /// Current paper mode.
    /// Returns `None` when engines have not been started yet (e.g. restore
    /// failed at boot, or no bots exist). Returning `None` — rather than a
    /// silent `false` — prevents the frontend from mistaking "engine not
    /// running" for "live trading mode".
    fn paper_mode(&self) -> Option<bool>;

    /// Restore error message — if `restore_if_needed` failed at boot, this
    /// returns the error string so the API/frontend can surface it.
    /// `None` means no restore has been attempted or it succeeded.
    fn restore_error(&self) -> Option<String>;

    /// Register a symbol for paper mode price ticks
    async fn register_paper_symbol(&self, exchange: String, symbol: String);

    /// 订阅 Position Engine 事件（用于 /ws/position 推送）
    fn pe_event_subscribe(&self) -> Option<broadcast::Receiver<EngineEvent>>;

    /// 查询 PE 中指定 symbol 的当前仓位快照（用于 /ws/position subscribe 时推送）
    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<virs_types::position::Position>;

    /// Restore services if bots exist in DB but engines are not started.
    /// Called once at server startup. No-op if engines already started or no bots exist.
    async fn restore_if_needed(&self) -> VirsResult<()>;

    /// Gracefully shut down all trading engines (Position, Grid, Auto).
    /// Called during application shutdown to ensure clean exit.
    async fn shutdown(&self);
}

/// API 应用状态
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
    /// HTTP 请求超时（秒）— 从 TimeConfig.http_timeout_secs 注入，
    /// 用于创建交易所客户端时的超时设置。
    pub http_timeout_secs: u64,
}

impl FromRef<AppState> for sqlx::PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.db_pool.clone()
    }
}
