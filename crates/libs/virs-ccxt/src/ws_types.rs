use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;


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


#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OrderBookLevel {
    pub price: f64,
    pub amount: f64,
}


#[derive(Debug, Clone)]
pub struct WsOrderBookUpdate {
    pub symbol: String,

    pub bids: Vec<OrderBookLevel>,

    pub asks: Vec<OrderBookLevel>,


    pub timestamp: i64,

    pub last_update_id: Option<i64>,
}


#[derive(Debug, Clone)]
pub enum WsOrderBookEvent {
    OrderBook(WsOrderBookUpdate),
    Reconnected,
}


#[async_trait]
pub trait OrderBookWsClient: Send + Sync {
    async fn start(&mut self, update_tx: broadcast::Sender<WsOrderBookEvent>);
    async fn stop(&mut self);
    async fn subscribe(&self, symbol: &str);
    async fn unsubscribe(&self, symbol: &str);
    fn is_running(&self) -> bool;
}
