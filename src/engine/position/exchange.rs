use async_trait::async_trait;
use tokio::sync::mpsc;

use super::error::Result;
use super::types::*;

#[async_trait]
pub trait Exchange: Send + Sync {
    fn name(&self) -> &str;
    fn market_type(&self) -> MarketType;

    // 行情数据
    async fn get_ticker(&self, symbol: &str) -> Result<Ticker>;
    async fn get_balance(&self) -> Result<Balance>;
    async fn get_positions(&self, symbol: Option<&str>) -> Result<Vec<ExchangePosition>>;
    async fn get_funding_rate(&self, symbol: &str) -> Result<FundingRate>;
    async fn get_fee_rates(&self, symbol: &str) -> Result<FeeRates>;

    // 交易
    async fn place_order(&self, params: PlaceOrderParams) -> Result<Order>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order>;
    async fn cancel_all_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>>;
    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>>;
    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order>;

    // 永续合约特有
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<()>;
    async fn get_position_mode(&self) -> Result<PositionMode>;

    // WebSocket 成交回报
    async fn subscribe_order_updates(&self, symbols: &[&str]) -> Result<mpsc::Receiver<WsFeedEvent>>;

    /// 价格 tick — Paper 模式下驱动 Limit 单撮合
    /// 真实交易所实现为 no-op（由 WebSocket 推送订单更新）
    async fn on_price_tick(&self, _symbol: &str, _price: f64) {}
}
