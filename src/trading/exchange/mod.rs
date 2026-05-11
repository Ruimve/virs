//! Exchange adapter layer.
//!
//! This module provides a thin adapter between the application's internal
//! `Exchange` trait (used by engine/api) and the CCXT-style exchange layer
//! (`crate::trading::ccxt` module). This keeps the ccxt module isolated while
//! allowing the rest of the application to use a simplified interface.

pub mod binance_position_adapter;
pub mod registry;

#[cfg(test)]
mod test;

use async_trait::async_trait;
use crate::models::*;
use crate::trading::ccxt::{self, Exchange as CcxtExchange, PlaceOrderParams, MarketType as CcxtMarketType};

// Re-export the old Exchange trait and factory for backward compatibility
// with engine and other modules that use it.

/// Unified exchange trait — adapter to the ccxt layer.
/// All exchange implementations must implement this trait.
#[async_trait]
pub trait Exchange: Send + Sync {
    /// Return the exchange identifier (e.g., "binance", "okx").
    fn name(&self) -> &str;

    /// Return the market type this exchange instance is bound to.
    fn market_type(&self) -> MarketType;

    // ---- Market Data ----

    /// Fetch the latest ticker for a symbol.
    async fn get_ticker(&self, symbol: &str) -> anyhow::Result<Ticker>;

    /// Fetch kline/candlestick data.
    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        since: Option<i64>,
    ) -> anyhow::Result<Vec<Kline>>;

    /// Fetch klines for a full time range [start_ms, end_ms] with automatic pagination.
    async fn get_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> anyhow::Result<Vec<Kline>>;

    /// Fetch the order book.
    async fn get_order_book(&self, symbol: &str, depth: u32) -> anyhow::Result<OrderBook>;

    /// Fetch account balances.
    async fn get_balances(&self) -> anyhow::Result<Vec<Balance>>;

    // ---- Trading ----

    /// Place a new order using the exchange's bound market type.
    async fn place_order(
        &self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        amount: f64,
        price: Option<f64>,
    ) -> anyhow::Result<Order> {
        self.place_order_with_options(symbol, side, order_type, amount, price, None, None).await
    }

    /// Place an order with optional reduce_only and position_side (for perpetual contracts).
    async fn place_order_with_options(
        &self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        amount: f64,
        price: Option<f64>,
        reduce_only: Option<bool>,
        position_side: Option<PositionSide>,
    ) -> anyhow::Result<Order>;

    /// Cancel an existing order.
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order>;

    /// Get order status.
    async fn get_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order>;

    /// Get open orders for a symbol.
    async fn get_open_orders(&self, symbol: Option<&str>) -> anyhow::Result<Vec<Order>>;

    // ---- Market Info ----

    /// Get available trading symbols for this exchange's bound market type.
    async fn get_symbols(&self) -> anyhow::Result<Vec<String>>;

    /// Check if exchange is healthy / reachable.
    async fn ping(&self) -> anyhow::Result<bool>;

    // ---- Perpetual Contracts ----

    /// Set leverage for a perpetual contract.
    /// Returns error if this is a spot exchange.
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> anyhow::Result<()>;

    /// Fetch current positions from exchange.
    /// Returns error if this is a spot exchange.
    async fn get_positions(&self, symbol: Option<&str>) -> anyhow::Result<Vec<ExchangePosition>>;

    /// Fetch funding rate for a perpetual contract.
    /// Returns error if this is a spot exchange.
    async fn get_funding_rate(&self, symbol: &str) -> anyhow::Result<FundingRate>;

    /// Fetch historical funding rates for a perpetual contract.
    async fn get_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> anyhow::Result<Vec<FundingHistoryEntry>>;
}

#[async_trait]
impl Exchange for Box<dyn Exchange> {
    fn name(&self) -> &str {
        (**self).name()
    }

    fn market_type(&self) -> MarketType {
        (**self).market_type()
    }

    async fn get_ticker(&self, symbol: &str) -> anyhow::Result<Ticker> {
        (**self).get_ticker(symbol).await
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        since: Option<i64>,
    ) -> anyhow::Result<Vec<Kline>> {
        (**self).get_klines(symbol, interval, limit, since).await
    }

    async fn get_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> anyhow::Result<Vec<Kline>> {
        (**self).get_klines_range(symbol, interval, start_ms, end_ms).await
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
    ) -> anyhow::Result<Order> {
        (**self).place_order(symbol, side, order_type, amount, price).await
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
    ) -> anyhow::Result<Order> {
        (**self).place_order_with_options(symbol, side, order_type, amount, price, reduce_only, position_side).await
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

    async fn get_symbols(&self) -> anyhow::Result<Vec<String>> {
        (**self).get_symbols().await
    }

    async fn ping(&self) -> anyhow::Result<bool> {
        (**self).ping().await
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> anyhow::Result<()> {
        (**self).set_leverage(symbol, leverage).await
    }

    async fn get_positions(&self, symbol: Option<&str>) -> anyhow::Result<Vec<ExchangePosition>> {
        (**self).get_positions(symbol).await
    }

    async fn get_funding_rate(&self, symbol: &str) -> anyhow::Result<FundingRate> {
        (**self).get_funding_rate(symbol).await
    }

    async fn get_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> anyhow::Result<Vec<FundingHistoryEntry>> {
        (**self).get_funding_history(symbol, start_time, end_time).await
    }
}

/// Adapter that wraps a ccxt Exchange into the application's Exchange trait.
/// Each adapter is bound to a specific market type (Spot or Perpetual),
/// ensuring that trading operations are isolated.
pub struct CcxtAdapter {
    inner: Box<dyn CcxtExchange>,
    market_type: MarketType,
}

impl CcxtAdapter {
    pub fn new(exchange: Box<dyn CcxtExchange>, market_type: MarketType) -> Self {
        Self { inner: exchange, market_type }
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
        CcxtMarketType::Perpetual => MarketType::Perpetual,
    }
}

/// Convert models MarketType to ccxt MarketType.
fn to_ccxt_market_type(mt: &MarketType) -> CcxtMarketType {
    match mt {
        MarketType::Spot => CcxtMarketType::Spot,
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
    let interval_ms = match interval {
        "1m" => 60_000,
        "5m" => 300_000,
        "15m" => 900_000,
        "30m" => 1_800_000,
        "1h" => 3_600_000,
        "4h" => 14_400_000,
        "1d" => 86_400_000,
        "1w" => 604_800_000,
        _ => 3_600_000, // default to 1h
    };
    Kline {
        open_time: ck.timestamp,
        open: ck.open,
        high: ck.high,
        low: ck.low,
        close: ck.close,
        volume: ck.volume,
        close_time: ck.timestamp + interval_ms,
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
        cost: co.cost,
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

    fn market_type(&self) -> MarketType {
        self.market_type.clone()
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
        since: Option<i64>,
    ) -> anyhow::Result<Vec<Kline>> {
        let cks = self.inner.fetch_ohlcv(symbol, interval, limit, since).await
            .map_err(|e| anyhow::anyhow!("ccxt ohlcv error: {}", e))?;
        let exchange_name = self.inner.id();
        Ok(cks.into_iter()
            .map(|ck| to_models_kline(ck, symbol, exchange_name, interval))
            .collect())
    }

    async fn get_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> anyhow::Result<Vec<Kline>> {
        let cks = self.inner.fetch_ohlcv_range(symbol, interval, start_ms, end_ms).await
            .map_err(|e| anyhow::anyhow!("ccxt ohlcv range error: {}", e))?;
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
        tracing::info!("[CcxtAdapter::get_balances] Calling inner.fetch_balance()...");
        let cbs = self.inner.fetch_balance().await
            .map_err(|e| anyhow::anyhow!("ccxt balance error: {}", e))?;
        tracing::info!("[CcxtAdapter::get_balances] fetch_balance returned {} balances", cbs.len());
        Ok(cbs.into_iter().map(to_models_balance).collect())
    }

    async fn place_order(
        &self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        amount: f64,
        price: Option<f64>,
    ) -> anyhow::Result<Order> {
        self.place_order_with_options(symbol, side, order_type, amount, price, None, None).await
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
    ) -> anyhow::Result<Order> {
        let ccxt_position_side = position_side.map(|ps| match ps {
            PositionSide::Long => ccxt::types::PositionSide::Long,
            PositionSide::Short => ccxt::types::PositionSide::Short,
        });
        let params = PlaceOrderParams {
            symbol: symbol.to_string(),
            side: to_ccxt_side(&side),
            order_type: to_ccxt_order_type(&order_type),
            amount,
            price,
            market_type: to_ccxt_market_type(&self.market_type),
            client_order_id: None,
            stop_price: None,
            time_in_force: None,
            reduce_only,
            leverage: None,
            margin_mode: None,
            position_side: ccxt_position_side,
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

    async fn get_symbols(&self) -> anyhow::Result<Vec<String>> {
        let markets = self.inner.fetch_markets().await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_markets error: {}", e))?;
        let ccxt_mt = to_ccxt_market_type(&self.market_type);
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

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> anyhow::Result<()> {
        self.inner.set_leverage(symbol, leverage, ccxt::types::MarginMode::Cross).await
            .map_err(|e| anyhow::anyhow!("ccxt set_leverage error: {}", e))
    }

    async fn get_positions(&self, symbol: Option<&str>) -> anyhow::Result<Vec<ExchangePosition>> {
        let positions = self.inner.fetch_positions(symbol).await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_positions error: {}", e))?;
        Ok(positions.into_iter().map(|p| ExchangePosition {
            symbol: p.symbol,
            side: match p.side {
                ccxt::types::PositionSide::Long => PositionSide::Long,
                ccxt::types::PositionSide::Short => PositionSide::Short,
            },
            size: p.size,
            entry_price: p.entry_price,
            leverage: p.leverage,
            unrealized_pnl: p.unrealized_pnl,
            liquidation_price: p.liquidation_price,
        }).collect())
    }

    async fn get_funding_rate(&self, symbol: &str) -> anyhow::Result<FundingRate> {
        let fr = self.inner.fetch_funding_rate(symbol).await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_funding_rate error: {}", e))?;
        Ok(FundingRate {
            symbol: fr.symbol,
            rate: fr.rate,
            next_funding_time: fr.next_funding_time,
        })
    }

    async fn get_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> anyhow::Result<Vec<FundingHistoryEntry>> {
        let entries = self.inner.fetch_funding_history(symbol, start_time, end_time).await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_funding_history error: {}", e))?;
        Ok(entries.into_iter().map(|e| FundingHistoryEntry {
            funding_time: e.funding_time,
            rate: e.rate,
        }).collect())
    }
}

/// Factory to create exchange instances by name and market type.
/// Delegates to the ccxt module's create_exchange function.
pub struct ExchangeFactory;

impl ExchangeFactory {
    /// Create an exchange instance from the given name, credentials, and market type.
    pub fn create(
        name: &str,
        api_key: &str,
        api_secret: &str,
        passphrase: Option<&str>,
        proxy_url: Option<&str>,
        market_type: MarketType,
    ) -> anyhow::Result<Box<dyn Exchange>> {
        let ccxt_ex = ccxt::create_exchange(name, api_key, api_secret, passphrase, proxy_url, &to_ccxt_market_type(&market_type))
            .map_err(|e| anyhow::anyhow!("Failed to create exchange '{}': {}", name, e))?;
        Ok(Box::new(CcxtAdapter::new(ccxt_ex, market_type)))
    }

    /// Create a K-line WebSocket client for Binance.
    /// Returns a client that implements `engine::kline::types::KlineWsClient`.
    pub fn create_binance_kline_ws(
        market_type: MarketType,
        proxy_url: Option<&str>,
    ) -> std::sync::Arc<tokio::sync::Mutex<dyn crate::engine::kline::types::KlineWsClient>> {
        use crate::trading::ccxt::adapter::binance::kline_ws::BinanceKlineWs;
        let ws = match market_type {
            MarketType::Spot => BinanceKlineWs::new_spot(proxy_url),
            MarketType::Perpetual => BinanceKlineWs::new_perpetual(proxy_url),
        };
        std::sync::Arc::new(tokio::sync::Mutex::new(ws))
    }

    /// Create a K-line data source (HTTP-based).
    /// Returns a source that implements `engine::kline::types::KlineSource`.
    pub fn create_kline_source(proxy_url: Option<String>) -> std::sync::Arc<dyn crate::engine::kline::types::KlineSource> {
        std::sync::Arc::new(crate::trading::ccxt::kline_source::CcxtKlineSource::new(proxy_url))
    }
}
