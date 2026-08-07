use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::Candle;

/* WebSocket K 线更新事件 */
#[derive(Debug, Clone)]
pub struct WsCandleUpdate {
    pub symbol: String,
    pub candle: Candle,
}

/* WebSocket 事件枚举：K 线更新或重连通知 */
#[derive(Debug, Clone)]
pub enum WsEvent {
    Candle(WsCandleUpdate),
    Reconnected,
}

/* K 线 WebSocket 客户端 trait：定义订阅、取消订阅和生命周期管理接口 */
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


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrderBookEvent {
    pub exchange: String,
    pub symbol: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: i64,
}

/*
 * WebSocket 订单簿更新事件：包含买卖盘快照和 last_update_id（用于同步校验，
 * 确保本地订单簿与交易所一致，避免增量更新丢失）
 */
#[derive(Debug, Clone)]
pub struct WsOrderBookUpdate {
    pub symbol: String,

    pub bids: Vec<OrderBookLevel>,

    pub asks: Vec<OrderBookLevel>,

    pub timestamp: i64,

    /* 最后更新 ID：用于订单簿同步校验 */
    pub last_update_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum WsOrderBookEvent {
    OrderBook(WsOrderBookUpdate),
    Reconnected,
}

/* 订单簿 WebSocket 客户端 trait：定义订单簿数据的订阅和生命周期管理接口 */
#[async_trait]
pub trait OrderBookWsClient: Send + Sync {
    async fn start(&mut self, update_tx: broadcast::Sender<WsOrderBookEvent>);
    async fn stop(&mut self);
    async fn subscribe(&self, symbol: &str);
    async fn unsubscribe(&self, symbol: &str);
    fn is_running(&self) -> bool;
}
