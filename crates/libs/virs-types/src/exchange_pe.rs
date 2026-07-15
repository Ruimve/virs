use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;

use virs_error::VirsResult;

use crate::ccxt_order::OrderResult;
use crate::enums::*;
use crate::market::*;
use crate::position::*;


pub type OrderUpdateStream = Pin<Box<dyn Stream<Item = WsFeedEvent> + Send>>;


#[async_trait]
pub trait ExchangePe: Send + Sync {
    fn name(&self) -> &str;
    fn market_type(&self) -> MarketType;


    async fn get_ticker(&self, symbol: &str) -> VirsResult<Ticker>;
    async fn get_balance(&self) -> VirsResult<Balance>;
    async fn get_positions(&self, symbol: Option<&str>) -> VirsResult<Vec<ExchangePosition>>;
    async fn get_funding_rate(&self, symbol: &str) -> VirsResult<FundingRate>;


    async fn place_order(&self, params: PlaceOrderParams) -> VirsResult<OrderResult>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> VirsResult<OrderResult>;
    async fn cancel_all_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<OrderResult>>;


    async fn set_leverage(&self, symbol: &str, leverage: u32) -> VirsResult<()>;
    async fn get_position_mode(&self) -> VirsResult<PositionMode>;


    async fn subscribe_order_updates(&self, symbols: &[&str]) -> VirsResult<OrderUpdateStream>;


    async fn on_price_tick(&self, _symbol: &str, _price: f64) {}


    async fn restore_positions(&self, _positions: Vec<ExchangePosition>) {}
}
