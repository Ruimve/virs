//! Exchange PE trait — used by the Position Engine.
//!
//! This is the low-level exchange interface for position management,
//! order placement, and balance queries. Implemented by CcctExchangeAdapter
//! and PaperExchangeAdapter.

use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;

use virs_error::VirsResult;

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
    async fn get_ticker(&self, symbol: &str) -> VirsResult<Ticker>;
    async fn get_balance(&self) -> VirsResult<Balance>;
    async fn get_positions(&self, symbol: Option<&str>) -> VirsResult<Vec<ExchangePosition>>;
    async fn get_funding_rate(&self, symbol: &str) -> VirsResult<FundingRate>;

    // Trading
    async fn place_order(&self, params: PlaceOrderParams) -> VirsResult<PositionOrder>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> VirsResult<PositionOrder>;
    async fn cancel_all_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<PositionOrder>>;
    async fn get_open_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<PositionOrder>>;
    async fn get_order(&self, symbol: &str, order_id: &str) -> VirsResult<PositionOrder>;

    // Perpetual contracts
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> VirsResult<()>;
    async fn get_position_mode(&self) -> VirsResult<PositionMode>;

    // WebSocket order updates
    async fn subscribe_order_updates(&self, symbols: &[&str]) -> VirsResult<OrderUpdateStream>;

    /// Price tick — Paper mode drives Limit order matching.
    /// Real exchange implementations should be no-op (WebSocket pushes order updates).
    async fn on_price_tick(&self, _symbol: &str, _price: f64) {}

    /// 从 DB 恢复仓位到交易所内存状态（仅 Paper 模式需要）。
    /// 真实交易所无需实现（仓位状态由交易所维护，重启不丢失）。
    /// PE 在 recover_state 时调用，避免 PE 误判"本地有但交易所没有"。
    async fn restore_positions(&self, _positions: Vec<ExchangePosition>) {}
}
