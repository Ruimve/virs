//! Exchange adapter layer.
//!
//! This crate provides:
//! - Application-level `Exchange` trait (wrapping ccxt)
//! - `CcxtAdapter` — adapts ccxt Exchange to application Exchange trait
//! - `Exchanges` — registry for named exchange instances
//! - `PaperExchangeAdapter` — paper trading adapter for Position Engine
//! - `CcxtExchangeAdapter` — real exchange adapter for Position Engine

pub mod adapter;
pub mod paper;
pub mod pe_adapter;
pub mod registry;

// Re-export key types
pub use adapter::CcxtAdapter;
pub use paper::PaperExchangeAdapter;
pub use pe_adapter::CcxtExchangeAdapter;
pub use registry::Exchanges;

// ============================================================
// Test modules (_tests suffix pattern)
// ============================================================
#[cfg(test)]
mod adapter_tests;
#[cfg(test)]
mod paper_tests;
#[cfg(test)]
mod pe_adapter_tests;

use async_trait::async_trait;
use virs_error::ExchangeError;
use virs_models::*;

/// Unified exchange trait — adapter to the ccxt layer.
/// All exchange implementations must implement this trait.
#[async_trait]
pub trait Exchange: Send + Sync {
    fn name(&self) -> &str;
    fn market_type(&self) -> MarketType;

    // ---- Market Data ----
    async fn get_ticker(&self, symbol: &str) -> Result<Ticker, ExchangeError>;
    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<Kline>, ExchangeError>;
    async fn get_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<Kline>, ExchangeError>;
    async fn get_order_book(&self, symbol: &str, depth: u32) -> Result<OrderBook, ExchangeError>;
    async fn get_balances(&self) -> Result<Vec<Balance>, ExchangeError>;

    // ---- Trading ----
    async fn place_order(
        &self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        amount: f64,
        price: Option<f64>,
    ) -> Result<Order, ExchangeError> {
        self.place_order_with_options(symbol, side, order_type, amount, price, None, None)
            .await
    }
    async fn place_order_with_options(
        &self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        amount: f64,
        price: Option<f64>,
        reduce_only: Option<bool>,
        position_side: Option<PositionSide>,
    ) -> Result<Order, ExchangeError>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError>;
    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError>;
    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>, ExchangeError>;

    // ---- Market Info ----
    async fn get_symbols(&self) -> Result<Vec<String>, ExchangeError>;
    async fn get_min_qty(&self, symbol: &str) -> Result<f64, ExchangeError>;
    async fn ping(&self) -> Result<bool, ExchangeError>;

    // ---- Perpetual Contracts ----
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<(), ExchangeError>;
    async fn get_positions(&self, symbol: Option<&str>) -> Result<Vec<ExchangePosition>, ExchangeError>;
    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError>;
    async fn get_funding_rate(&self, symbol: &str) -> Result<FundingRate, ExchangeError>;
    async fn get_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<FundingHistoryEntry>, ExchangeError>;
    async fn create_listen_key(&self) -> Result<String, ExchangeError>;
    async fn keepalive_listen_key(&self, listen_key: &str) -> Result<(), ExchangeError>;

    // ---- Account ----
    async fn get_api_restrictions(&self) -> Result<virs_ccxt::ApiRestrictions, ExchangeError>;

    // ---- WebSocket API (现货 Ed25519 用户数据流) ----
    /// 启动现货用户数据流 WebSocket API，返回订单事件 receiver。
    ///
    /// 仅当交易所支持 Ed25519 签名时可用（如 Binance 现货 + Ed25519 API Key）。
    /// 不支持时返回 `Err`，调用方应降级到 listenKey 方案。
    async fn start_spot_order_ws_api(
        &self,
    ) -> Result<tokio::sync::mpsc::Receiver<virs_types::WsFeedEvent>, ExchangeError>;

    /// 启动基于 listenKey 的订单 WebSocket（合约用户数据流，或现货 HMAC 降级路径）。
    ///
    /// 实现内部完成 listenKey 创建/复用 + WS 客户端构造与启动，返回事件 receiver。
    /// `listen_key_hint` 传入已缓存的 listenKey 可避免重复创建。
    /// 不支持时返回 `Err`。
    async fn start_listenkey_order_ws(
        &self,
        listen_key_hint: Option<&str>,
    ) -> Result<tokio::sync::mpsc::Receiver<virs_types::WsFeedEvent>, ExchangeError>;
}

#[async_trait]
impl Exchange for Box<dyn Exchange> {
    fn name(&self) -> &str {
        (**self).name()
    }
    fn market_type(&self) -> MarketType {
        (**self).market_type()
    }
    async fn get_ticker(&self, symbol: &str) -> Result<Ticker, ExchangeError> {
        (**self).get_ticker(symbol).await
    }
    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<Kline>, ExchangeError> {
        (**self).get_klines(symbol, interval, limit, since).await
    }
    async fn get_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<Kline>, ExchangeError> {
        (**self)
            .get_klines_range(symbol, interval, start_ms, end_ms)
            .await
    }
    async fn get_order_book(&self, symbol: &str, depth: u32) -> Result<OrderBook, ExchangeError> {
        (**self).get_order_book(symbol, depth).await
    }
    async fn get_balances(&self) -> Result<Vec<Balance>, ExchangeError> {
        (**self).get_balances().await
    }
    async fn place_order(
        &self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        amount: f64,
        price: Option<f64>,
    ) -> Result<Order, ExchangeError> {
        (**self)
            .place_order(symbol, side, order_type, amount, price)
            .await
    }
    async fn place_order_with_options(
        &self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        amount: f64,
        price: Option<f64>,
        reduce_only: Option<bool>,
        position_side: Option<PositionSide>,
    ) -> Result<Order, ExchangeError> {
        (**self)
            .place_order_with_options(
                symbol,
                side,
                order_type,
                amount,
                price,
                reduce_only,
                position_side,
            )
            .await
    }
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        (**self).cancel_order(symbol, order_id).await
    }
    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        (**self).get_order(symbol, order_id).await
    }
    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>, ExchangeError> {
        (**self).get_open_orders(symbol).await
    }
    async fn get_symbols(&self) -> Result<Vec<String>, ExchangeError> {
        (**self).get_symbols().await
    }
    async fn get_min_qty(&self, symbol: &str) -> Result<f64, ExchangeError> {
        (**self).get_min_qty(symbol).await
    }
    async fn ping(&self) -> Result<bool, ExchangeError> {
        (**self).ping().await
    }
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<(), ExchangeError> {
        (**self).set_leverage(symbol, leverage).await
    }
    async fn get_positions(&self, symbol: Option<&str>) -> Result<Vec<ExchangePosition>, ExchangeError> {
        (**self).get_positions(symbol).await
    }
    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError> {
        (**self).get_position_mode().await
    }
    async fn get_funding_rate(&self, symbol: &str) -> Result<FundingRate, ExchangeError> {
        (**self).get_funding_rate(symbol).await
    }
    async fn get_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<FundingHistoryEntry>, ExchangeError> {
        (**self)
            .get_funding_history(symbol, start_time, end_time)
            .await
    }
    async fn create_listen_key(&self) -> Result<String, ExchangeError> {
        (**self).create_listen_key().await
    }
    async fn keepalive_listen_key(&self, listen_key: &str) -> Result<(), ExchangeError> {
        (**self).keepalive_listen_key(listen_key).await
    }
    async fn get_api_restrictions(&self) -> Result<virs_ccxt::ApiRestrictions, ExchangeError> {
        (**self).get_api_restrictions().await
    }
    async fn start_spot_order_ws_api(
        &self,
    ) -> Result<tokio::sync::mpsc::Receiver<virs_types::WsFeedEvent>, ExchangeError> {
        (**self).start_spot_order_ws_api().await
    }
    async fn start_listenkey_order_ws(
        &self,
        listen_key_hint: Option<&str>,
    ) -> Result<tokio::sync::mpsc::Receiver<virs_types::WsFeedEvent>, ExchangeError> {
        (**self).start_listenkey_order_ws(listen_key_hint).await
    }
}
