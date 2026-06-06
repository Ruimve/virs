//! Exchange PE trait — used by the Position Engine.
//!
//! This is the low-level exchange interface for position management,
//! order placement, and balance queries. Implemented by CcctExchangeAdapter
//! and PaperExchangeAdapter.

use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;

use crate::enums::*;
use crate::market::*;
use crate::position::*;

/// Type alias for the order update stream returned by subscribe_order_updates.
pub type OrderUpdateStream = Pin<Box<dyn Stream<Item = WsFeedEvent> + Send>>;

/// Position Engine Exchange trait.
#[async_trait]
pub trait ExchangePe: Send + Sync {
    fn name(&self) -> &str;
    fn market_type(&self) -> MarketType;

    // Market data
    async fn get_ticker(&self, symbol: &str) -> PositionResult<Ticker>;
    async fn get_balance(&self) -> PositionResult<Balance>;
    async fn get_positions(&self, symbol: Option<&str>) -> PositionResult<Vec<ExchangePosition>>;
    async fn get_funding_rate(&self, symbol: &str) -> PositionResult<FundingRate>;
    async fn get_fee_rates(&self, symbol: &str) -> PositionResult<FeeRates>;

    // Trading
    async fn place_order(&self, params: PlaceOrderParams) -> PositionResult<PositionOrder>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> PositionResult<PositionOrder>;
    async fn cancel_all_orders(&self, symbol: Option<&str>) -> PositionResult<Vec<PositionOrder>>;
    async fn get_open_orders(&self, symbol: Option<&str>) -> PositionResult<Vec<PositionOrder>>;
    async fn get_order(&self, symbol: &str, order_id: &str) -> PositionResult<PositionOrder>;

    // Perpetual contracts
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> PositionResult<()>;
    async fn get_position_mode(&self) -> PositionResult<PositionMode>;

    // WebSocket order updates
    async fn subscribe_order_updates(&self, symbols: &[&str]) -> PositionResult<OrderUpdateStream>;

    /// Price tick — Paper mode drives Limit order matching.
    /// Real exchange implementations should be no-op (WebSocket pushes order updates).
    async fn on_price_tick(&self, _symbol: &str, _price: f64) {}
}
