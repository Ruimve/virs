use async_trait::async_trait;
use tokio_stream::wrappers::ReceiverStream;
use virs_ccxt::{self, Exchange as CcxtExchange, PlaceOrderParams as CcxtPlaceOrderParams};
use virs_error::{ExchangeError, VirsResult};
use virs_types::enums::*;
use virs_types::exchange_pe::{ExchangePe, OrderUpdateStream};
use virs_types::market::*;
use virs_types::position::PlaceOrderParams;
use virs_types::OrderResult;

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

/// 将 CcxtKline 转换为 Kline，补充 symbol/exchange/interval 字段。
///
/// CcxtKline 是 ccxt 层的原始 K 线数据（无 symbol/exchange/interval），
/// Kline 是业务层完整 K 线，需要补充这三个上下文字段。
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

#[async_trait]
impl ExchangePe for CcxtAdapter {
    fn name(&self) -> &str {
        self.inner.id()
    }

    fn market_type(&self) -> MarketType {
        self.market_type
    }

    async fn get_ticker(&self, symbol: &str) -> VirsResult<Ticker> {
        let ct = self.inner.fetch_ticker(symbol).await?;
        ct.try_into().map_err(Into::into)
    }

    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
        since: Option<i64>,
    ) -> VirsResult<Vec<Kline>> {
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
    ) -> VirsResult<Vec<Kline>> {
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

    async fn get_balance(&self) -> VirsResult<Balance> {
        let balances = self.inner.fetch_balance().await?;
        let usdt = balances
            .iter()
            .find(|b| b.asset.eq_ignore_ascii_case("USDT"))
            .ok_or_else(|| {
                ExchangeError::NoData(
                    "USDT balance not found in exchange balances — cannot return 0.0 as it would bypass risk checks".to_string(),
                )
            })?;
        Ok(Balance {
            asset: "USDT".to_string(),
            free: usdt.free,
            used: usdt.used,
            total: usdt.total,
        })
    }

    async fn get_positions(&self, symbol: Option<&str>) -> VirsResult<Vec<ExchangePosition>> {
        // virs_ccxt::ExchangePosition 已与 virs_types::ExchangePosition 统一（同一类型），
        // 无需任何转换，直接返回。
        self.inner
            .fetch_positions(symbol)
            .await
            .map_err(Into::into)
    }

    async fn get_funding_rate(&self, symbol: &str) -> VirsResult<FundingRate> {
        let fr = self.inner.fetch_funding_rate(symbol).await?;
        Ok(fr.into())
    }

    async fn get_symbols(&self) -> VirsResult<Vec<String>> {
        let markets = self.get_markets_cached().await?;
        Ok(markets
            .into_iter()
            .filter(|m| m.market_type == self.market_type && m.active)
            .map(|m| m.symbol)
            .collect())
    }

    async fn get_min_qty(&self, symbol: &str) -> VirsResult<f64> {
        let markets = self.get_markets_cached().await?;
        let found = markets
            .iter()
            .find(|m| m.symbol == symbol || m.id == symbol);
        match found {
            Some(m) => Ok(m.min_amount.ok_or_else(|| {
                ExchangeError::NoData(format!(
                    "symbol {} found but min_amount is None — exchange did not return minimum order amount",
                    symbol
                ))
            })?),
            None => Err(ExchangeError::NoData(format!(
                "symbol {} not found in markets",
                symbol
            ))
            .into()),
        }
    }

    async fn place_order(&self, params: PlaceOrderParams) -> VirsResult<OrderResult> {
        // 从 virs_types::PlaceOrderParams（10 字段）构造 virs_ccxt::PlaceOrderParams（12 字段）。
        // 补充 adapter 层独有字段：market_type（来自自身）、leverage=None、margin_mode=Cross。
        let ccxt_params = CcxtPlaceOrderParams {
            symbol: params.symbol,
            side: params.side,
            order_type: params.order_type,
            amount: params.amount,
            price: params.price,
            market_type: self.market_type,
            client_order_id: params.client_order_id,
            stop_price: params.stop_price,
            time_in_force: params.time_in_force,
            leverage: None,
            margin_mode: Some(MarginMode::Cross),
            position_side: params.position_side,
        };
        self.inner
            .create_order(ccxt_params)
            .await
            .map_err(Into::into)
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> VirsResult<OrderResult> {
        self.inner
            .cancel_order(symbol, order_id)
            .await
            .map_err(Into::into)
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<OrderResult>> {
        // ccxt 层 cancel_all_orders 需要 symbol（Binance DELETE /fapi/v1/allOpenOrders 必填），
        // None 时返回错误。ccxt 返回 ()，这里返回空 Vec。
        let sym = symbol.ok_or_else(|| {
            ExchangeError::InvalidRequest(
                "cancel_all_orders requires a symbol for DELETE /fapi/v1/allOpenOrders".into(),
            )
        })?;
        self.inner.cancel_all_orders(sym).await?;
        Ok(Vec::new())
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> VirsResult<()> {
        // ExchangePe 的 set_leverage 不接收 margin_mode，内部固定使用 Cross。
        self.inner
            .set_leverage(symbol, leverage, MarginMode::Cross)
            .await
            .map_err(Into::into)
    }

    async fn get_position_mode(&self) -> VirsResult<PositionMode> {
        self.inner
            .get_position_mode()
            .await
            .map_err(Into::into)
    }

    async fn create_listen_key(&self) -> VirsResult<String> {
        self.inner
            .create_listen_key()
            .await
            .map_err(Into::into)
    }

    async fn ping(&self) -> VirsResult<bool> {
        self.inner.ping().await.map_err(Into::into)
    }

    async fn get_api_restrictions(&self) -> VirsResult<ApiRestrictions> {
        self.inner
            .fetch_api_restrictions()
            .await
            .map_err(Into::into)
    }

    async fn subscribe_order_updates(&self, _symbols: &[&str]) -> VirsResult<OrderUpdateStream> {
        // ccxt start_listenkey_order_ws 返回 mpsc::Receiver<WsFeedEvent>，
        // 用 ReceiverStream 包装后转为 OrderUpdateStream（Pin<Box<dyn Stream>>）。
        let rx = self.inner.start_listenkey_order_ws(None).await?;
        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
