//! WebSocket and kline-related types for virs-ccxt.
//!
//! These types are used by the Binance WebSocket clients (kline_ws, order_ws).
//! They mirror the types from the original engine modules but are self-contained
//! within the ccxt crate to avoid circular dependencies.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

// ---- Kline types ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub open_time: i64,
    pub close_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: f64,
    pub trades: i64,
    pub closed: bool,
}

#[derive(Debug, Clone)]
pub struct WsCandleUpdate {
    pub symbol: String,
    pub candle: Candle,
}

#[derive(Debug, Clone)]
pub enum WsEvent {
    Candle(WsCandleUpdate),
    Reconnected,
}

#[async_trait]
pub trait KlineWsClient: Send + Sync {
    async fn start(&mut self, update_tx: broadcast::Sender<WsEvent>);
    async fn stop(&mut self);
    async fn subscribe(&self, symbol: &str);
    async fn unsubscribe(&self, symbol: &str);
    fn is_running(&self) -> bool;
}

// ---- Position Engine WS types ----

/// WebSocket feed event from exchange order updates.
#[derive(Debug, Clone)]
pub enum WsFeedEvent {
    OrderUpdate {
        exchange_order_id: String,
        symbol: String,
        status: virs_types::OrderStatus,
        filled: f64,
        remaining: f64,
        price: f64,
        amount: f64,
        commission: f64,
        timestamp: chrono::DateTime<chrono::Utc>,
        position_side: Option<virs_types::PositionSide>,
    },
    ConnectionChanged {
        connected: bool,
    },
}
