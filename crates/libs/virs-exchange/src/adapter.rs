//! CcxtAdapter — wraps a ccxt Exchange into the application's Exchange trait.

use async_trait::async_trait;
use virs_ccxt::{self, Exchange as CcxtExchange, PlaceOrderParams};
use virs_error::ExchangeError;
use virs_models::*;

use crate::Exchange;

/// Adapter that wraps a ccxt Exchange into the application's Exchange trait.
pub struct CcxtAdapter {
    inner: Box<dyn CcxtExchange>,
    market_type: MarketType,
    markets_cache: tokio::sync::RwLock<Option<Vec<virs_ccxt::MarketInfo>>>,
}

impl CcxtAdapter {
    pub fn new(exchange: Box<dyn CcxtExchange>, market_type: MarketType) -> Self {
        Self {
            inner: exchange,
            market_type,
            markets_cache: tokio::sync::RwLock::new(None),
        }
    }

    async fn get_markets_cached(&self) -> Result<Vec<virs_ccxt::MarketInfo>, ExchangeError> {
        {
            let cache = self.markets_cache.read().await;
            if cache.is_some() {
                return Ok(cache.as_ref().unwrap().clone());
            }
        }
        let markets = self.inner.fetch_markets().await.map_err(|e| {
            tracing::error!(error = %e, "fetch_markets failed");
            e
        })?;
        let mut cache = self.markets_cache.write().await;
        *cache = Some(markets.clone());
        Ok(markets)
    }
}

// ---- Type conversion helpers ----

pub fn to_ccxt_market_type(mt: &MarketType) -> virs_ccxt::MarketType {
    match mt {
        MarketType::Spot => virs_ccxt::MarketType::Spot,
        MarketType::Perpetual => virs_ccxt::MarketType::Perpetual,
    }
}

pub fn to_ccxt_side(side: &Side) -> virs_ccxt::Side {
    match side {
        Side::Buy => virs_ccxt::Side::Buy,
        Side::Sell => virs_ccxt::Side::Sell,
    }
}

pub fn to_ccxt_order_type(ot: &OrderType) -> virs_ccxt::OrderType {
    match ot {
        OrderType::Market => virs_ccxt::OrderType::Market,
        OrderType::Limit => virs_ccxt::OrderType::Limit,
        OrderType::StopMarket => virs_ccxt::OrderType::StopMarket,
        OrderType::StopLimit => virs_ccxt::OrderType::StopLimit,
        OrderType::TakeProfitMarket => virs_ccxt::OrderType::TakeProfitMarket,
    }
}

pub fn to_models_kline(
    ck: virs_ccxt::CcxtKline,
    symbol: &str,
    exchange: &str,
    interval: &str,
) -> Kline {
    let interval_ms = match interval {
        "1m" => 60_000,
        "5m" => 300_000,
        "15m" => 900_000,
        "30m" => 1_800_000,
        "1h" => 3_600_000,
        "4h" => 14_400_000,
        "1d" => 86_400_000,
        "1w" => 604_800_000,
        _ => 3_600_000,
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

pub fn to_models_balance(cb: virs_ccxt::Balance) -> Balance {
    Balance {
        asset: cb.asset,
        free: cb.free,
        used: cb.used,
        total: cb.total,
    }
}

pub fn to_models_order(co: virs_ccxt::CcxtOrder) -> Order {
    let fee_info = co.fee.as_ref();
    Order {
        id: co.id,
        client_order_id: co.client_order_id,
        symbol: co.symbol,
        side: co.side,
        order_type: co.order_type,
        price: co.price,
        amount: co.amount,
        cost: co.cost,
        filled: co.filled,
        remaining: co.remaining,
        status: co.status.into(),
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
        self.market_type
    }

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker, ExchangeError> {
        let ct = self.inner.fetch_ticker(symbol).await?;
        Ok(ct.into())
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<Kline>, ExchangeError> {
        let cks = self
            .inner
            .fetch_ohlcv(symbol, interval, limit, since)
            .await?;
        let exchange_name = self.inner.id();
        Ok(cks
            .into_iter()
            .map(|ck| to_models_kline(ck, symbol, exchange_name, interval))
            .collect())
    }

    async fn get_klines_range(
        &self,
        symbol: &str,
        interval: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<Kline>, ExchangeError> {
        let cks = self
            .inner
            .fetch_ohlcv_range(symbol, interval, start_ms, end_ms)
            .await?;
        let exchange_name = self.inner.id();
        Ok(cks
            .into_iter()
            .map(|ck| to_models_kline(ck, symbol, exchange_name, interval))
            .collect())
    }

    async fn get_order_book(&self, symbol: &str, depth: u32) -> Result<OrderBook, ExchangeError> {
        let cob = self.inner.fetch_order_book(symbol, depth).await?;
        Ok(cob.into())
    }

    async fn get_balances(&self) -> Result<Vec<Balance>, ExchangeError> {
        let cbs = self.inner.fetch_balance().await?;
        Ok(cbs.into_iter().map(to_models_balance).collect())
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
        let ccxt_position_side = position_side.map(|ps| match ps {
            PositionSide::Long => virs_ccxt::PositionSide::Long,
            PositionSide::Short => virs_ccxt::PositionSide::Short,
            PositionSide::Both => virs_ccxt::PositionSide::Both,
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
        let co = self.inner.create_order(params).await?;
        Ok(to_models_order(co))
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        let co = self.inner.cancel_order(symbol, order_id).await?;
        Ok(to_models_order(co))
    }

    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        let co = self.inner.fetch_order(symbol, order_id).await?;
        Ok(to_models_order(co))
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>, ExchangeError> {
        let cos = self.inner.fetch_open_orders(symbol).await?;
        Ok(cos.into_iter().map(to_models_order).collect())
    }

    async fn get_symbols(&self) -> Result<Vec<String>, ExchangeError> {
        let markets = self.get_markets_cached().await?;
        let ccxt_mt = to_ccxt_market_type(&self.market_type);
        Ok(markets
            .into_iter()
            .filter(|m| m.market_type == ccxt_mt && m.active)
            .map(|m| m.symbol)
            .collect())
    }

    async fn get_min_qty(&self, symbol: &str) -> Result<f64, ExchangeError> {
        let markets = self.get_markets_cached().await?;
        let found = markets
            .iter()
            .find(|m| m.symbol == symbol || m.id == symbol);
        match found {
            Some(m) => Ok(m.min_amount.unwrap_or(0.0)),
            None => Err(ExchangeError::NoData(format!(
                "symbol {} not found in markets",
                symbol
            ))),
        }
    }

    async fn ping(&self) -> Result<bool, ExchangeError> {
        self.inner.ping().await
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<(), ExchangeError> {
        self.inner
            .set_leverage(symbol, leverage, virs_ccxt::MarginMode::Cross)
            .await
    }

    async fn get_positions(&self, symbol: Option<&str>) -> Result<Vec<ExchangePosition>, ExchangeError> {
        let positions = self.inner.fetch_positions(symbol).await?;
        Ok(positions
            .into_iter()
            .map(|p| ExchangePosition {
                symbol: p.symbol,
                side: match p.side {
                    virs_ccxt::PositionSide::Long => PositionSide::Long,
                    virs_ccxt::PositionSide::Short => PositionSide::Short,
                    virs_ccxt::PositionSide::Both => PositionSide::Both,
                },
                size: p.size,
                entry_price: p.entry_price,
                leverage: p.leverage,
                unrealized_pnl: p.unrealized_pnl,
                liquidation_price: p.liquidation_price,
            })
            .collect())
    }

    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError> {
        let mode = self.inner.get_position_mode().await?;
        Ok(match mode {
            virs_ccxt::PositionMode::OneWay => PositionMode::OneWay,
            virs_ccxt::PositionMode::Hedge => PositionMode::Hedge,
        })
    }

    async fn get_funding_rate(&self, symbol: &str) -> Result<FundingRate, ExchangeError> {
        let fr = self.inner.fetch_funding_rate(symbol).await?;
        Ok(fr.into())
    }

    async fn get_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<FundingHistoryEntry>, ExchangeError> {
        let entries = self
            .inner
            .fetch_funding_history(symbol, start_time, end_time)
            .await?;
        Ok(entries.into_iter().map(|e| e.into()).collect())
    }

    async fn create_listen_key(&self) -> Result<String, ExchangeError> {
        self.inner.create_listen_key().await
    }

    async fn keepalive_listen_key(&self, listen_key: &str) -> Result<(), ExchangeError> {
        self.inner.keepalive_listen_key(listen_key).await
    }

    async fn get_api_restrictions(&self) -> Result<virs_ccxt::ApiRestrictions, ExchangeError> {
        self.inner.fetch_api_restrictions().await
    }

    async fn start_spot_order_ws_api(
        &self,
    ) -> Result<tokio::sync::mpsc::Receiver<virs_ccxt::WsFeedEvent>, ExchangeError> {
        self.inner.start_spot_order_ws_api().await
    }

    async fn start_listenkey_order_ws(
        &self,
        listen_key_hint: Option<&str>,
    ) -> Result<tokio::sync::mpsc::Receiver<virs_ccxt::WsFeedEvent>, ExchangeError> {
        self.inner.start_listenkey_order_ws(listen_key_hint).await
    }
}
