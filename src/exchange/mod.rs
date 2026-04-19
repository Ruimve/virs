//! Exchange adapter layer.
//!
//! This module provides a thin adapter between the application's internal
//! `Exchange` trait (used by engine/api) and the CCXT-style exchange layer
//! (`crate::ccxt` module). This keeps the ccxt module isolated while
//! allowing the rest of the application to use a simplified interface.

use async_trait::async_trait;
use crate::models::*;
use crate::ccxt::{self, Exchange as CcxtExchange, PlaceOrderParams, MarketType as CcxtMarketType};

// Re-export the old Exchange trait and factory for backward compatibility
// with engine and other modules that use it.

/// Unified exchange trait — adapter to the ccxt layer.
/// All exchange implementations must implement this trait.
#[async_trait]
pub trait Exchange: Send + Sync {
    /// Return the exchange identifier (e.g., "binance", "okx").
    fn name(&self) -> &str;

    // ---- Market Data ----

    /// Fetch the latest ticker for a symbol.
    async fn get_ticker(&self, symbol: &str) -> anyhow::Result<Ticker>;

    /// Fetch kline/candlestick data.
    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        end_time: Option<i64>,
    ) -> anyhow::Result<Vec<Kline>>;

    /// Fetch the order book.
    async fn get_order_book(&self, symbol: &str, depth: u32) -> anyhow::Result<OrderBook>;

    /// Fetch account balances.
    async fn get_balances(&self) -> anyhow::Result<Vec<Balance>>;

    // ---- Trading ----

    /// Place a new order.
    async fn place_order(
        &self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        amount: f64,
        price: Option<f64>,
        market_type: MarketType,
    ) -> anyhow::Result<Order>;

    /// Cancel an existing order.
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order>;

    /// Get order status.
    async fn get_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order>;

    /// Get open orders for a symbol.
    async fn get_open_orders(&self, symbol: Option<&str>) -> anyhow::Result<Vec<Order>>;

    // ---- Market Info ----

    /// Get available trading symbols.
    async fn get_symbols(&self, market_type: MarketType) -> anyhow::Result<Vec<String>>;

    /// Check if exchange is healthy / reachable.
    async fn ping(&self) -> anyhow::Result<bool>;
}

#[async_trait]
impl Exchange for Box<dyn Exchange> {
    fn name(&self) -> &str {
        (**self).name()
    }

    async fn get_ticker(&self, symbol: &str) -> anyhow::Result<Ticker> {
        (**self).get_ticker(symbol).await
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        end_time: Option<i64>,
    ) -> anyhow::Result<Vec<Kline>> {
        (**self).get_klines(symbol, interval, limit, end_time).await
    }

    async fn get_order_book(&self, symbol: &str, depth: u32) -> anyhow::Result<OrderBook> {
        (**self).get_order_book(symbol, depth).await
    }

    async fn get_balances(&self) -> anyhow::Result<Vec<Balance>> {
        (**self).get_balances().await
    }

    async fn place_order(
        &self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        amount: f64,
        price: Option<f64>,
        market_type: MarketType,
    ) -> anyhow::Result<Order> {
        (**self).place_order(symbol, side, order_type, amount, price, market_type).await
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order> {
        (**self).cancel_order(symbol, order_id).await
    }

    async fn get_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order> {
        (**self).get_order(symbol, order_id).await
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> anyhow::Result<Vec<Order>> {
        (**self).get_open_orders(symbol).await
    }

    async fn get_symbols(&self, market_type: MarketType) -> anyhow::Result<Vec<String>> {
        (**self).get_symbols(market_type).await
    }

    async fn ping(&self) -> anyhow::Result<bool> {
        (**self).ping().await
    }
}

/// Adapter that wraps a ccxt Exchange into the application's Exchange trait.
pub struct CcxtAdapter {
    inner: Box<dyn CcxtExchange>,
}

impl CcxtAdapter {
    pub fn new(exchange: Box<dyn CcxtExchange>) -> Self {
        Self { inner: exchange }
    }

    /// Get a reference to the underlying ccxt exchange.
    pub fn ccxt(&self) -> &dyn CcxtExchange {
        self.inner.as_ref()
    }
}

/// Convert ccxt MarketType to models MarketType.
fn to_models_market_type(mt: &CcxtMarketType) -> MarketType {
    match mt {
        CcxtMarketType::Spot => MarketType::Spot,
        CcxtMarketType::Futures => MarketType::Futures,
        CcxtMarketType::Perpetual => MarketType::Perpetual,
        CcxtMarketType::Option => MarketType::Futures, // Not directly supported
    }
}

/// Convert models MarketType to ccxt MarketType.
fn to_ccxt_market_type(mt: &MarketType) -> CcxtMarketType {
    match mt {
        MarketType::Spot => CcxtMarketType::Spot,
        MarketType::Futures => CcxtMarketType::Futures,
        MarketType::Perpetual => CcxtMarketType::Perpetual,
    }
}

/// Convert ccxt Side to models Side.
fn to_models_side(side: &ccxt::Side) -> Side {
    match side {
        ccxt::Side::Buy => Side::Buy,
        ccxt::Side::Sell => Side::Sell,
    }
}

/// Convert models Side to ccxt Side.
fn to_ccxt_side(side: &Side) -> ccxt::Side {
    match side {
        Side::Buy => ccxt::Side::Buy,
        Side::Sell => ccxt::Side::Sell,
    }
}

/// Convert ccxt OrderType to models OrderType.
fn to_models_order_type(ot: &ccxt::OrderType) -> OrderType {
    match ot {
        ccxt::OrderType::Market => OrderType::Market,
        ccxt::OrderType::Limit => OrderType::Limit,
        ccxt::OrderType::StopMarket => OrderType::StopMarket,
        ccxt::OrderType::StopLimit => OrderType::StopLimit,
    }
}

/// Convert models OrderType to ccxt OrderType.
fn to_ccxt_order_type(ot: &OrderType) -> ccxt::OrderType {
    match ot {
        OrderType::Market => ccxt::OrderType::Market,
        OrderType::Limit => ccxt::OrderType::Limit,
        OrderType::StopMarket => ccxt::OrderType::StopMarket,
        OrderType::StopLimit => ccxt::OrderType::StopLimit,
    }
}

/// Convert ccxt OrderStatus to models OrderStatus.
fn to_models_order_status(os: &ccxt::OrderStatus) -> OrderStatus {
    match os {
        ccxt::OrderStatus::Open => OrderStatus::Open,
        ccxt::OrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
        ccxt::OrderStatus::Filled => OrderStatus::Filled,
        ccxt::OrderStatus::Canceled => OrderStatus::Canceled,
        ccxt::OrderStatus::Expired => OrderStatus::Canceled,
        ccxt::OrderStatus::Failed => OrderStatus::Failed,
        ccxt::OrderStatus::Rejected => OrderStatus::Failed,
    }
}

/// Convert ccxt Ticker to models Ticker.
fn to_models_ticker(ct: ccxt::Ticker) -> Ticker {
    Ticker {
        symbol: ct.symbol,
        exchange: ct.exchange,
        bid: ct.bid.unwrap_or(0.0),
        ask: ct.ask.unwrap_or(0.0),
        last: ct.last.unwrap_or(0.0),
        high_24h: ct.high.unwrap_or(0.0),
        low_24h: ct.low.unwrap_or(0.0),
        volume_24h: ct.volume.unwrap_or(0.0),
        price_change_24h: ct.price_change.unwrap_or(0.0),
        price_change_pct_24h: ct.price_change_pct.unwrap_or(0.0),
        timestamp: ct.timestamp.unwrap_or_else(chrono::Utc::now),
    }
}

/// Convert ccxt Kline to models Kline.
fn to_models_kline(ck: ccxt::Kline, symbol: &str, exchange: &str, interval: &str) -> Kline {
    Kline {
        open_time: ck.timestamp,
        open: ck.open,
        high: ck.high,
        low: ck.low,
        close: ck.close,
        volume: ck.volume,
        close_time: ck.timestamp + 3600, // approximate
        quote_volume: ck.quote_volume.unwrap_or(0.0),
        trades: ck.trades.unwrap_or(0),
        symbol: symbol.to_string(),
        exchange: exchange.to_string(),
        interval: interval.to_string(),
    }
}

/// Convert ccxt OrderBook to models OrderBook.
fn to_models_order_book(cob: ccxt::OrderBook) -> OrderBook {
    OrderBook {
        symbol: cob.symbol,
        bids: cob.bids,
        asks: cob.asks,
        timestamp: cob.timestamp.unwrap_or_else(chrono::Utc::now),
    }
}

/// Convert ccxt Balance to models Balance.
fn to_models_balance(cb: ccxt::Balance) -> Balance {
    Balance {
        asset: cb.asset,
        free: cb.free,
        used: cb.used,
        total: cb.total,
    }
}

/// Convert ccxt Order to models Order.
fn to_models_order(co: ccxt::Order) -> Order {
    let fee_info = co.fee.as_ref();
    Order {
        id: co.id,
        client_order_id: co.client_order_id,
        symbol: co.symbol,
        side: to_models_side(&co.side),
        order_type: to_models_order_type(&co.order_type),
        price: co.price,
        amount: co.amount,
        filled: co.filled,
        remaining: co.remaining,
        status: to_models_order_status(&co.status),
        fee: fee_info.map(|f| f.cost).unwrap_or(0.0),
        fee_currency: fee_info.map(|f| f.currency.clone()).unwrap_or_default(),
        created_at: co.created_at.unwrap_or_else(chrono::Utc::now),
        updated_at: co.updated_at.unwrap_or_else(chrono::Utc::now),
    }
}

#[async_trait]
impl Exchange for CcxtAdapter {
    fn name(&self) -> &str {
        self.inner.id()
    }

    async fn get_ticker(&self, symbol: &str) -> anyhow::Result<Ticker> {
        let ct = self.inner.fetch_ticker(symbol).await
            .map_err(|e| anyhow::anyhow!("ccxt ticker error: {}", e))?;
        Ok(to_models_ticker(ct))
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        end_time: Option<i64>,
    ) -> anyhow::Result<Vec<Kline>> {
        let cks = self.inner.fetch_ohlcv(symbol, interval, limit, end_time).await
            .map_err(|e| anyhow::anyhow!("ccxt ohlcv error: {}", e))?;
        let exchange_name = self.inner.id();
        Ok(cks.into_iter()
            .map(|ck| to_models_kline(ck, symbol, exchange_name, interval))
            .collect())
    }

    async fn get_order_book(&self, symbol: &str, depth: u32) -> anyhow::Result<OrderBook> {
        let cob = self.inner.fetch_order_book(symbol, depth).await
            .map_err(|e| anyhow::anyhow!("ccxt orderbook error: {}", e))?;
        Ok(to_models_order_book(cob))
    }

    async fn get_balances(&self) -> anyhow::Result<Vec<Balance>> {
        let cbs = self.inner.fetch_balance().await
            .map_err(|e| anyhow::anyhow!("ccxt balance error: {}", e))?;
        Ok(cbs.into_iter().map(to_models_balance).collect())
    }

    async fn place_order(
        &self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        amount: f64,
        price: Option<f64>,
        market_type: MarketType,
    ) -> anyhow::Result<Order> {
        let params = PlaceOrderParams {
            symbol: symbol.to_string(),
            side: to_ccxt_side(&side),
            order_type: to_ccxt_order_type(&order_type),
            amount,
            price,
            market_type: to_ccxt_market_type(&market_type),
            client_order_id: None,
            stop_price: None,
            time_in_force: None,
            reduce_only: None,
        };
        let co = self.inner.create_order(params).await
            .map_err(|e| anyhow::anyhow!("ccxt create_order error: {}", e))?;
        Ok(to_models_order(co))
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order> {
        let co = self.inner.cancel_order(symbol, order_id).await
            .map_err(|e| anyhow::anyhow!("ccxt cancel_order error: {}", e))?;
        Ok(to_models_order(co))
    }

    async fn get_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order> {
        let co = self.inner.fetch_order(symbol, order_id).await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_order error: {}", e))?;
        Ok(to_models_order(co))
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> anyhow::Result<Vec<Order>> {
        let cos = self.inner.fetch_open_orders(symbol).await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_open_orders error: {}", e))?;
        Ok(cos.into_iter().map(to_models_order).collect())
    }

    async fn get_symbols(&self, market_type: MarketType) -> anyhow::Result<Vec<String>> {
        let markets = self.inner.fetch_markets().await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_markets error: {}", e))?;
        let ccxt_mt = to_ccxt_market_type(&market_type);
        Ok(markets
            .into_iter()
            .filter(|m| m.market_type == ccxt_mt && m.active)
            .map(|m| m.symbol)
            .collect())
    }

    async fn ping(&self) -> anyhow::Result<bool> {
        self.inner.ping().await
            .map_err(|e| anyhow::anyhow!("ccxt ping error: {}", e))
    }
}

/// Factory to create exchange instances by name.
/// Delegates to the ccxt module's create_exchange function.
pub struct ExchangeFactory;

impl ExchangeFactory {
    /// Create an exchange instance from the given name and credentials.
    pub fn create(
        name: &str,
        api_key: &str,
        api_secret: &str,
        passphrase: Option<&str>,
        proxy_url: Option<&str>,
    ) -> anyhow::Result<Box<dyn Exchange>> {
        let ccxt_ex = ccxt::create_exchange(name, api_key, api_secret, passphrase, proxy_url)
            .map_err(|e| anyhow::anyhow!("Failed to create exchange '{}': {}", name, e))?;
        Ok(Box::new(CcxtAdapter::new(ccxt_ex)))
    }
}
