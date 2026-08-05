use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use virs_error::VirsResult;

use crate::exchange::{MarketType, PositionMode};
use crate::market::*;
use crate::order::OrderResult;
use crate::position::PlaceOrderParams;
use crate::ws_types::{KlineWsClient, OrderBookWsClient};
use super::structs::OrderUpdateStream;


/// 统一交易所接口 trait。
///
/// 底层交易所连接（如 `BinanceExchange`）直接实现本 trait，
/// 通过 `create_exchange()` 返回 `Box<dyn ExchangePe>`。
/// Paper 交易引擎通过 `PaperExchangeAdapter` 实现。
#[async_trait]
pub trait ExchangePe: Send + Sync {
    fn name(&self) -> &str;
    fn market_type(&self) -> MarketType;

    // ---- 行情 ----
    async fn get_ticker(&self, symbol: &str) -> VirsResult<Ticker>;
    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        since: Option<i64>,
    ) -> VirsResult<Vec<Kline>>;
    async fn get_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> VirsResult<Vec<Kline>>;

    // ---- 账户与市场 ----
    async fn get_balance(&self) -> VirsResult<Balance>;
    async fn get_positions(&self, symbol: Option<&str>) -> VirsResult<Vec<ExchangePosition>>;
    async fn get_funding_rate(&self, symbol: &str) -> VirsResult<FundingRate>;
    async fn get_symbols(&self) -> VirsResult<Vec<String>>;
    async fn get_min_qty(&self, symbol: &str) -> VirsResult<f64>;

    // ---- 交易 ----
    async fn place_order(&self, params: PlaceOrderParams) -> VirsResult<OrderResult>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> VirsResult<OrderResult>;
    async fn cancel_all_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<OrderResult>>;

    // ---- 配置 ----
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> VirsResult<()>;
    async fn get_position_mode(&self) -> VirsResult<PositionMode>;
    async fn create_listen_key(&self) -> VirsResult<String>;

    // ---- 健康检查 ----
    async fn ping(&self) -> VirsResult<bool>;
    async fn get_api_restrictions(&self) -> VirsResult<ApiRestrictions>;

    // ---- WebSocket ----
    async fn subscribe_order_updates(&self, symbols: &[&str]) -> VirsResult<OrderUpdateStream>;

    // ---- 回调（默认空实现） ----
    async fn on_price_tick(&self, _symbol: &str, _price: f64) {}
    async fn restore_positions(&self, _positions: Vec<ExchangePosition>) {}

    // ---- WS 工厂（默认返回 NotSupported） ----
    fn create_kline_ws(
        &self,
        _proxy: Option<&str>,
    ) -> VirsResult<Arc<Mutex<dyn KlineWsClient>>> {
        Err(virs_error::VirsError::Exchange(
            virs_error::ExchangeError::NotSupported("kline WS not supported".into()),
        ))
    }
    fn create_orderbook_ws(
        &self,
        _proxy: Option<&str>,
    ) -> VirsResult<Arc<Mutex<dyn OrderBookWsClient>>> {
        Err(virs_error::VirsError::Exchange(
            virs_error::ExchangeError::NotSupported("orderbook WS not supported".into()),
        ))
    }
}
