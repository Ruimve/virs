use std::sync::Arc;

use async_trait::async_trait;

use virs_error::VirsResult;
use virs_type::*;
use virs_type::{
    ApiRestrictions, Balance, ExchangePe, ExchangePosition, KlineWsClient, MarketType,
    OrderBookWsClient, OrderResult, OrderUpdateStream, PlaceOrderParams, PositionMode,
};

use crate::paper::PaperExchangeAdapter;


pub struct PaperModeExchange {
    real: Arc<dyn ExchangePe>,
    paper: PaperExchangeAdapter,
}

impl PaperModeExchange {


    pub fn new(real: Arc<dyn ExchangePe>, initial_balance: f64) -> Self {
        let paper = PaperExchangeAdapter::new(real.name(), real.market_type(), initial_balance);
        Self { real, paper }
    }
}

#[async_trait]
impl ExchangePe for PaperModeExchange {


    fn name(&self) -> &str {
        self.real.name()
    }

    fn market_type(&self) -> MarketType {
        self.real.market_type()
    }


    async fn get_ticker(&self, symbol: &str) -> VirsResult<Ticker> {
        self.real.get_ticker(symbol).await
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        since: Option<i64>,
    ) -> VirsResult<Vec<Kline>> {
        self.real.get_klines(symbol, interval, limit, since).await
    }

    async fn get_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> VirsResult<Vec<Kline>> {
        self.real
            .get_klines_range(symbol, interval, start_ms, end_ms)
            .await
    }

    async fn get_funding_rate(&self, symbol: &str) -> VirsResult<FundingRate> {
        self.real.get_funding_rate(symbol).await
    }

    async fn get_symbols(&self) -> VirsResult<Vec<String>> {
        self.real.get_symbols().await
    }

    async fn get_min_qty(&self, symbol: &str) -> VirsResult<f64> {
        self.real.get_min_qty(symbol).await
    }

    async fn ping(&self) -> VirsResult<bool> {
        self.real.ping().await
    }

    async fn get_api_restrictions(&self) -> VirsResult<ApiRestrictions> {
        self.real.get_api_restrictions().await
    }

    async fn get_position_mode(&self) -> VirsResult<PositionMode> {
        self.real.get_position_mode().await
    }

    async fn create_listen_key(&self) -> VirsResult<String> {
        self.real.create_listen_key().await
    }

    fn create_kline_ws(
        &self,
        proxy: Option<&str>,
    ) -> VirsResult<Arc<tokio::sync::Mutex<dyn KlineWsClient>>> {
        self.real.create_kline_ws(proxy)
    }

    fn create_orderbook_ws(
        &self,
        proxy: Option<&str>,
    ) -> VirsResult<Arc<tokio::sync::Mutex<dyn OrderBookWsClient>>> {
        self.real.create_orderbook_ws(proxy)
    }


    async fn get_balance(&self) -> VirsResult<Balance> {
        self.paper.get_balance().await
    }

    async fn get_positions(&self, symbol: Option<&str>) -> VirsResult<Vec<ExchangePosition>> {
        self.paper.get_positions(symbol).await
    }

    async fn place_order(&self, params: PlaceOrderParams) -> VirsResult<OrderResult> {
        self.paper.place_order(params).await
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> VirsResult<OrderResult> {
        self.paper.cancel_order(symbol, order_id).await
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<OrderResult>> {
        self.paper.cancel_all_orders(symbol).await
    }

    async fn subscribe_order_updates(&self, symbols: &[&str]) -> VirsResult<OrderUpdateStream> {
        self.paper.subscribe_order_updates(symbols).await
    }

    async fn on_price_tick(&self, symbol: &str, price: f64) {
        self.paper.on_price_tick(symbol, price).await;
    }

    async fn restore_positions(&self, positions: Vec<ExchangePosition>) {
        self.paper.restore_positions(positions).await;
    }


    async fn set_leverage(&self, symbol: &str, leverage: u32) -> VirsResult<()> {

        self.real.set_leverage(symbol, leverage).await?;

        self.paper.set_leverage(symbol, leverage).await?;
        Ok(())
    }
}
