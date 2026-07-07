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
// WsFeedEvent is now defined in virs_types::position and re-used directly.
// This eliminates the duplicate definition and the convert_ws_feed_event hop.

// ---- OrderBook WS types ----

/// A single price level in the order book.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OrderBookLevel {
    pub price: f64,
    pub amount: f64,
}

/// Order book snapshot pushed by the exchange WS.
#[derive(Debug, Clone)]
pub struct WsOrderBookUpdate {
    pub symbol: String,
    /// Top N bid levels (sorted descending by price)
    pub bids: Vec<OrderBookLevel>,
    /// Top N ask levels (sorted ascending by price)
    pub asks: Vec<OrderBookLevel>,
    /// Exchange event timestamp (ms).
    /// For perpetual: the `E` field (event time).
    /// For spot: 0 (spot partial book depth has no timestamp).
    pub timestamp: i64,
    /// Spot-only: the `lastUpdateId` field (a sequence number, NOT a timestamp).
    /// None for perpetual updates.
    pub last_update_id: Option<i64>,
}

/// Events emitted by an OrderBook WS client.
/// Mirrors `WsEvent` (kline) but for order book snapshots.
#[derive(Debug, Clone)]
pub enum WsOrderBookEvent {
    OrderBook(WsOrderBookUpdate),
    Reconnected,
}

/// Trait for exchange order book WebSocket clients.
/// Mirrors `KlineWsClient` but pushes top-N order book snapshots.
#[async_trait]
pub trait OrderBookWsClient: Send + Sync {
    async fn start(&mut self, update_tx: broadcast::Sender<WsOrderBookEvent>);
    async fn stop(&mut self);
    async fn subscribe(&self, symbol: &str);
    async fn unsubscribe(&self, symbol: &str);
    fn is_running(&self) -> bool;
}
