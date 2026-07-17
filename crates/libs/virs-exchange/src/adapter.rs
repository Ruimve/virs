use async_trait::async_trait;
use virs_ccxt::{self, Exchange as CcxtExchange, PlaceOrderParams};
use virs_error::ExchangeError;
use virs_models::*;

use crate::Exchange;


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
            if let Some(ref markets) = *cache {
                return Ok(markets.clone());
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


pub fn to_ccxt_market_type(mt: &MarketType) -> MarketType {
    match mt {
        MarketType::Perpetual => MarketType::Perpetual,
    }
}

pub fn to_ccxt_side(side: &Side) -> virs_ccxt::Side {
    match side {
        Side::Buy => virs_ccxt::Side::Buy,
        Side::Sell => virs_ccxt::Side::Sell,
    }
}

pub fn to_ccxt_order_type(ot: &OrderType) -> virs_ccxt::OrderType {
    // models::OrderType 与 virs_ccxt::OrderType 均为 virs_types::enums::OrderType 的重新导出，类型一致
    *ot
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
        close_time: ck.close_time.unwrap_or(ck.timestamp + interval_ms - 1),
        quote_volume: ck.quote_volume.unwrap_or_else(|| {
            tracing::warn!("Kline quote_volume is None — exchange did not provide this field, defaulting to 0.0");
            0.0
        }),
        trades: ck.trades.unwrap_or_else(|| {
            tracing::warn!("Kline trades count is None — exchange did not provide this field, defaulting to 0");
            0
        }),
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

pub fn to_models_order(result: virs_ccxt::OrderResult) -> Order {
    Order {
        id: result.order_id,
        client_order_id: Some(result.client_order_id),
        symbol: String::new(),
        side: Side::Buy,
        order_type: OrderType::Market,
        price: None,
        amount: 0.0,
        cost: None,
        filled: 0.0,
        remaining: 0.0,
        status: OrderStatus::Pending,
        fee: 0.0,
        fee_currency: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
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
        ct.try_into()
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
        position_side: Option<PositionSide>,
        client_order_id: Option<&str>,
    ) -> Result<Order, ExchangeError> {
        let ccxt_position_side = position_side.map(|ps| match ps {
            PositionSide::Long => virs_ccxt::PositionSide::Long,
            PositionSide::Short => virs_ccxt::PositionSide::Short,
        });
        let params = PlaceOrderParams {
            symbol: symbol.to_string(),
            side: to_ccxt_side(&side),
            order_type: to_ccxt_order_type(&order_type),
            amount,
            price,
            market_type: to_ccxt_market_type(&self.market_type),
            client_order_id: client_order_id.map(|s| s.to_string()),
            stop_price: None,
            time_in_force: None,
            leverage: None,
            margin_mode: None,
            position_side: ccxt_position_side,
        };
        let result = self.inner.create_order(params).await?;
        Ok(to_models_order(result))
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order, ExchangeError> {
        let result = self.inner.cancel_order(symbol, order_id).await?;
        Ok(to_models_order(result))
    }

    async fn cancel_all_orders(&self, symbol: &str) -> Result<(), ExchangeError> {
        self.inner.cancel_all_orders(symbol).await
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
            Some(m) => {
                m.min_amount.ok_or_else(|| {
                    ExchangeError::NoData(format!(
                        "symbol {} found but min_amount is None — exchange did not return minimum order amount",
                        symbol
                    ))
                })
            }
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
                },
                quantity: p.quantity,
                entry_price: p.entry_price,
                leverage: p.leverage,
            })
            .collect())
    }

    async fn get_position_mode(&self) -> Result<PositionMode, ExchangeError> {


        self.inner.get_position_mode().await
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

    async fn start_listenkey_order_ws(
        &self,
        listen_key_hint: Option<&str>,
    ) -> Result<tokio::sync::mpsc::Receiver<virs_types::WsFeedEvent>, ExchangeError> {
        self.inner.start_listenkey_order_ws(listen_key_hint).await
    }
}
