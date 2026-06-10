//! Exchange adapter layer.
//!
//! This crate provides:
//! - Application-level `Exchange` trait (wrapping ccxt)
//! - `CcxtAdapter` — adapts ccxt Exchange to application Exchange trait
//! - `ExchangeRegistry` — registry for named exchange instances
//! - `PaperExchangeAdapter` — paper trading adapter for Position Engine
//! - `CcxtExchangeAdapter` — real exchange adapter for Position Engine

pub mod adapter;
pub mod registry;
pub mod paper;
pub mod pe_adapter;

// Re-export key types
pub use registry::ExchangeRegistry;
pub use adapter::CcxtAdapter;
pub use paper::PaperExchangeAdapter;
pub use pe_adapter::CcxtExchangeAdapter;

use async_trait::async_trait;
use virs_models::*;

/// Unified exchange trait — adapter to the ccxt layer.
/// All exchange implementations must implement this trait.
#[async_trait]
pub trait Exchange: Send + Sync {
    fn name(&self) -> &str;
    fn market_type(&self) -> MarketType;

    // ---- Market Data ----
    async fn get_ticker(&self, symbol: &str) -> anyhow::Result<Ticker>;
    async fn get_klines(&self, symbol: &str, interval: &str, limit: u32, since: Option<i64>) -> anyhow::Result<Vec<Kline>>;
    async fn get_klines_range(&self, symbol: &str, interval: &str, start_ms: i64, end_ms: i64) -> anyhow::Result<Vec<Kline>>;
    async fn get_order_book(&self, symbol: &str, depth: u32) -> anyhow::Result<OrderBook>;
    async fn get_balances(&self) -> anyhow::Result<Vec<Balance>>;

    // ---- Trading ----
    async fn place_order(&self, symbol: &str, side: Side, order_type: OrderType, amount: f64, price: Option<f64>) -> anyhow::Result<Order> {
        self.place_order_with_options(symbol, side, order_type, amount, price, None, None).await
    }
    async fn place_order_with_options(
        &self, symbol: &str, side: Side, order_type: OrderType, amount: f64, price: Option<f64>,
        reduce_only: Option<bool>, position_side: Option<PositionSide>,
    ) -> anyhow::Result<Order>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order>;
    async fn get_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order>;
    async fn get_open_orders(&self, symbol: Option<&str>) -> anyhow::Result<Vec<Order>>;

    // ---- Market Info ----
    async fn get_symbols(&self) -> anyhow::Result<Vec<String>>;
    async fn get_min_qty(&self, symbol: &str) -> anyhow::Result<f64>;
    async fn ping(&self) -> anyhow::Result<bool>;

    // ---- Perpetual Contracts ----
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> anyhow::Result<()>;
    async fn get_positions(&self, symbol: Option<&str>) -> anyhow::Result<Vec<ExchangePosition>>;
    async fn get_position_mode(&self) -> anyhow::Result<PositionMode>;
    async fn get_funding_rate(&self, symbol: &str) -> anyhow::Result<FundingRate>;
    async fn get_funding_history(&self, symbol: &str, start_time: i64, end_time: i64) -> anyhow::Result<Vec<FundingHistoryEntry>>;
    async fn create_listen_key(&self) -> anyhow::Result<String>;
    async fn keepalive_listen_key(&self, listen_key: &str) -> anyhow::Result<()>;

    // ---- Account ----
    async fn get_api_restrictions(&self) -> anyhow::Result<virs_ccxt::ApiRestrictions>;
}

#[async_trait]
impl Exchange for Box<dyn Exchange> {
    fn name(&self) -> &str { (**self).name() }
    fn market_type(&self) -> MarketType { (**self).market_type() }
    async fn get_ticker(&self, symbol: &str) -> anyhow::Result<Ticker> { (**self).get_ticker(symbol).await }
    async fn get_klines(&self, symbol: &str, interval: &str, limit: u32, since: Option<i64>) -> anyhow::Result<Vec<Kline>> { (**self).get_klines(symbol, interval, limit, since).await }
    async fn get_klines_range(&self, symbol: &str, interval: &str, start_ms: i64, end_ms: i64) -> anyhow::Result<Vec<Kline>> { (**self).get_klines_range(symbol, interval, start_ms, end_ms).await }
    async fn get_order_book(&self, symbol: &str, depth: u32) -> anyhow::Result<OrderBook> { (**self).get_order_book(symbol, depth).await }
    async fn get_balances(&self) -> anyhow::Result<Vec<Balance>> { (**self).get_balances().await }
    async fn place_order(&self, symbol: &str, side: Side, order_type: OrderType, amount: f64, price: Option<f64>) -> anyhow::Result<Order> { (**self).place_order(symbol, side, order_type, amount, price).await }
    async fn place_order_with_options(&self, symbol: &str, side: Side, order_type: OrderType, amount: f64, price: Option<f64>, reduce_only: Option<bool>, position_side: Option<PositionSide>) -> anyhow::Result<Order> { (**self).place_order_with_options(symbol, side, order_type, amount, price, reduce_only, position_side).await }
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order> { (**self).cancel_order(symbol, order_id).await }
    async fn get_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order> { (**self).get_order(symbol, order_id).await }
    async fn get_open_orders(&self, symbol: Option<&str>) -> anyhow::Result<Vec<Order>> { (**self).get_open_orders(symbol).await }
    async fn get_symbols(&self) -> anyhow::Result<Vec<String>> { (**self).get_symbols().await }
    async fn get_min_qty(&self, symbol: &str) -> anyhow::Result<f64> { (**self).get_min_qty(symbol).await }
    async fn ping(&self) -> anyhow::Result<bool> { (**self).ping().await }
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> anyhow::Result<()> { (**self).set_leverage(symbol, leverage).await }
    async fn get_positions(&self, symbol: Option<&str>) -> anyhow::Result<Vec<ExchangePosition>> { (**self).get_positions(symbol).await }
    async fn get_position_mode(&self) -> anyhow::Result<PositionMode> { (**self).get_position_mode().await }
    async fn get_funding_rate(&self, symbol: &str) -> anyhow::Result<FundingRate> { (**self).get_funding_rate(symbol).await }
    async fn get_funding_history(&self, symbol: &str, start_time: i64, end_time: i64) -> anyhow::Result<Vec<FundingHistoryEntry>> { (**self).get_funding_history(symbol, start_time, end_time).await }
    async fn create_listen_key(&self) -> anyhow::Result<String> { (**self).create_listen_key().await }
    async fn keepalive_listen_key(&self, listen_key: &str) -> anyhow::Result<()> { (**self).keepalive_listen_key(listen_key).await }
    async fn get_api_restrictions(&self) -> anyhow::Result<virs_ccxt::ApiRestrictions> { (**self).get_api_restrictions().await }
}
