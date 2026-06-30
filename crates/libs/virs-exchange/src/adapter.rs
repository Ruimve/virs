//! CcxtAdapter — wraps a ccxt Exchange into the application's Exchange trait.

use async_trait::async_trait;
use virs_ccxt::{self, Exchange as CcxtExchange, PlaceOrderParams};
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

    async fn get_markets_cached(&self) -> anyhow::Result<Vec<virs_ccxt::MarketInfo>> {
        {
            let cache = self.markets_cache.read().await;
            if cache.is_some() {
                return Ok(cache.as_ref().unwrap().clone());
            }
        }
        let markets = self.inner.fetch_markets().await.map_err(|e| {
            tracing::error!(error = %e, "fetch_markets failed");
            anyhow::anyhow!("ccxt fetch_markets error: {}", e)
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

    async fn get_ticker(&self, symbol: &str) -> anyhow::Result<Ticker> {
        let ct = self
            .inner
            .fetch_ticker(symbol)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt ticker error: {}", e))?;
        Ok(ct.into())
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        since: Option<i64>,
    ) -> anyhow::Result<Vec<Kline>> {
        let cks = self
            .inner
            .fetch_ohlcv(symbol, interval, limit, since)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt ohlcv error: {}", e))?;
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
    ) -> anyhow::Result<Vec<Kline>> {
        let cks = self
            .inner
            .fetch_ohlcv_range(symbol, interval, start_ms, end_ms)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt ohlcv range error: {}", e))?;
        let exchange_name = self.inner.id();
        Ok(cks
            .into_iter()
            .map(|ck| to_models_kline(ck, symbol, exchange_name, interval))
            .collect())
    }

    async fn get_order_book(&self, symbol: &str, depth: u32) -> anyhow::Result<OrderBook> {
        let cob = self
            .inner
            .fetch_order_book(symbol, depth)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt orderbook error: {}", e))?;
        Ok(cob.into())
    }

    async fn get_balances(&self) -> anyhow::Result<Vec<Balance>> {
        let cbs = self
            .inner
            .fetch_balance()
            .await
            .map_err(|e| anyhow::anyhow!("ccxt balance error: {}", e))?;
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
    ) -> anyhow::Result<Order> {
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
        let co = self
            .inner
            .create_order(params)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt create_order error: {}", e))?;
        Ok(to_models_order(co))
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order> {
        let co = self
            .inner
            .cancel_order(symbol, order_id)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt cancel_order error: {}", e))?;
        Ok(to_models_order(co))
    }

    async fn get_order(&self, symbol: &str, order_id: &str) -> anyhow::Result<Order> {
        let co = self
            .inner
            .fetch_order(symbol, order_id)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_order error: {}", e))?;
        Ok(to_models_order(co))
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> anyhow::Result<Vec<Order>> {
        let cos = self
            .inner
            .fetch_open_orders(symbol)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_open_orders error: {}", e))?;
        Ok(cos.into_iter().map(to_models_order).collect())
    }

    async fn get_symbols(&self) -> anyhow::Result<Vec<String>> {
        let markets = self.get_markets_cached().await?;
        let ccxt_mt = to_ccxt_market_type(&self.market_type);
        Ok(markets
            .into_iter()
            .filter(|m| m.market_type == ccxt_mt && m.active)
            .map(|m| m.symbol)
            .collect())
    }

    async fn get_min_qty(&self, symbol: &str) -> anyhow::Result<f64> {
        let markets = self.get_markets_cached().await?;
        let found = markets
            .iter()
            .find(|m| m.symbol == symbol || m.id == symbol);
        match found {
            Some(m) => Ok(m.min_amount.unwrap_or(0.0)),
            None => {
                tracing::warn!(symbol = %symbol, total_markets = markets.len(), "Symbol not found in markets, returning 0.0 for min_qty");
                Ok(0.0)
            }
        }
    }

    async fn ping(&self) -> anyhow::Result<bool> {
        self.inner
            .ping()
            .await
            .map_err(|e| anyhow::anyhow!("ccxt ping error: {}", e))
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> anyhow::Result<()> {
        self.inner
            .set_leverage(symbol, leverage, virs_ccxt::MarginMode::Cross)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt set_leverage error: {}", e))
    }

    async fn get_positions(&self, symbol: Option<&str>) -> anyhow::Result<Vec<ExchangePosition>> {
        let positions = self
            .inner
            .fetch_positions(symbol)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_positions error: {}", e))?;
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

    async fn get_position_mode(&self) -> anyhow::Result<PositionMode> {
        let mode = self
            .inner
            .get_position_mode()
            .await
            .map_err(|e| anyhow::anyhow!("ccxt get_position_mode error: {}", e))?;
        Ok(match mode {
            virs_ccxt::PositionMode::OneWay => PositionMode::OneWay,
            virs_ccxt::PositionMode::Hedge => PositionMode::Hedge,
        })
    }

    async fn get_funding_rate(&self, symbol: &str) -> anyhow::Result<FundingRate> {
        let fr = self
            .inner
            .fetch_funding_rate(symbol)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_funding_rate error: {}", e))?;
        Ok(fr.into())
    }

    async fn get_funding_history(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> anyhow::Result<Vec<FundingHistoryEntry>> {
        let entries = self
            .inner
            .fetch_funding_history(symbol, start_time, end_time)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_funding_history error: {}", e))?;
        Ok(entries.into_iter().map(|e| e.into()).collect())
    }

    async fn create_listen_key(&self) -> anyhow::Result<String> {
        self.inner
            .create_listen_key()
            .await
            .map_err(|e| anyhow::anyhow!("ccxt create_listen_key error: {}", e))
    }

    async fn keepalive_listen_key(&self, listen_key: &str) -> anyhow::Result<()> {
        self.inner
            .keepalive_listen_key(listen_key)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt keepalive_listen_key error: {}", e))
    }

    async fn get_api_restrictions(&self) -> anyhow::Result<virs_ccxt::ApiRestrictions> {
        self.inner
            .fetch_api_restrictions()
            .await
            .map_err(|e| anyhow::anyhow!("ccxt fetch_api_restrictions error: {}", e))
    }

    async fn start_spot_order_ws_api(
        &self,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<virs_ccxt::WsFeedEvent>> {
        self.inner
            .start_spot_order_ws_api()
            .await
            .map_err(|e| anyhow::anyhow!("ccxt start_spot_order_ws_api error: {}", e))
    }

    async fn start_listenkey_order_ws(
        &self,
        listen_key_hint: Option<&str>,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<virs_ccxt::WsFeedEvent>> {
        self.inner
            .start_listenkey_order_ws(listen_key_hint)
            .await
            .map_err(|e| anyhow::anyhow!("ccxt start_listenkey_order_ws error: {}", e))
    }
}
