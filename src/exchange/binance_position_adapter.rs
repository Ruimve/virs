use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::mpsc;
use tracing::{warn, info};

use crate::ccxt::binance_order_ws::BinanceOrderWs;
use crate::exchange::Exchange as VirsExchange;
use crate::models;
use crate::engine::position::exchange::Exchange as PeExchange;
use crate::engine::position::types::*;
use crate::engine::position::error::Result;

/// 适配器：将 VIRS 现有的 Exchange trait 适配为 Position Engine 的 Exchange trait
///
/// 位于 src/exchange/ 下，引用 crate::position 和 crate::exchange
pub struct CcxtExchangeAdapter {
    inner: Box<dyn VirsExchange>,
    /// 可选的 WS listenKey，用于启动订单推送
    listen_key: Option<String>,
}

impl CcxtExchangeAdapter {
    pub fn new(exchange: Box<dyn VirsExchange>) -> Self {
        Self {
            inner: exchange,
            listen_key: None,
        }
    }

    /// 设置 listenKey 以启用 WebSocket 订单推送
    pub fn with_listen_key(mut self, listen_key: String) -> Self {
        self.listen_key = Some(listen_key);
        self
    }
}

// ── 类型转换辅助函数（pub(crate) 以便单元测试） ──

pub(crate) fn convert_side(side: &models::Side) -> Side {
    match side {
        models::Side::Buy => Side::Buy,
        models::Side::Sell => Side::Sell,
    }
}

pub(crate) fn convert_to_virs_side(side: &Side) -> models::Side {
    match side {
        Side::Buy => models::Side::Buy,
        Side::Sell => models::Side::Sell,
    }
}

pub(crate) fn convert_position_side(side: &Option<PositionSide>) -> Option<models::PositionSide> {
    side.as_ref().map(|s| match s {
        PositionSide::Long => models::PositionSide::Long,
        PositionSide::Short => models::PositionSide::Short,
        PositionSide::Both => models::PositionSide::Long,
    })
}

pub(crate) fn convert_order_type(ot: &OrderType) -> models::OrderType {
    match ot {
        OrderType::Limit => models::OrderType::Limit,
        OrderType::Market => models::OrderType::Market,
        OrderType::StopMarket => models::OrderType::StopMarket,
        OrderType::TakeProfitMarket => models::OrderType::StopMarket,
    }
}

pub(crate) fn convert_order_status(status: &models::OrderStatus) -> OrderStatus {
    match status {
        models::OrderStatus::Pending => OrderStatus::Pending,
        models::OrderStatus::Open => OrderStatus::Open,
        models::OrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
        models::OrderStatus::Filled => OrderStatus::Filled,
        models::OrderStatus::Canceled => OrderStatus::Canceled,
        models::OrderStatus::Failed => OrderStatus::Failed,
    }
}

pub(crate) fn convert_virs_position_side(side: &models::PositionSide) -> PositionSide {
    match side {
        models::PositionSide::Long => PositionSide::Long,
        models::PositionSide::Short => PositionSide::Short,
    }
}

pub(crate) fn convert_virs_market_type(mt: &models::MarketType) -> MarketType {
    match mt {
        models::MarketType::Spot => MarketType::Spot,
        models::MarketType::Perpetual => MarketType::Perpetual,
    }
}

pub(crate) fn convert_order(o: &models::Order) -> Order {
    Order {
        id: uuid::Uuid::parse_str(&o.id).unwrap_or_else(|_| uuid::Uuid::new_v4()),
        position_id: uuid::Uuid::nil(),
        exchange_order_id: Some(o.id.clone()),
        client_order_id: o.client_order_id.clone(),
        exchange: String::new(),
        symbol: o.symbol.clone(),
        side: convert_side(&o.side),
        order_type: match o.order_type {
            models::OrderType::Limit => OrderType::Limit,
            models::OrderType::Market => OrderType::Market,
            models::OrderType::StopMarket => OrderType::StopMarket,
            models::OrderType::StopLimit => OrderType::StopMarket,
        },
        request_price: o.price,
        fill_price: if o.filled > 0.0 { o.price } else { None },
        amount: o.amount,
        filled: o.filled,
        remaining: o.remaining,
        status: convert_order_status(&o.status),
        reduce_only: false,
        fee: o.fee,
        fee_currency: o.fee_currency.clone(),
        slippage: None,
        created_at: o.created_at,
        updated_at: o.updated_at,
    }
}

pub(crate) fn convert_exchange_position(ep: &models::ExchangePosition) -> ExchangePosition {
    ExchangePosition {
        symbol: ep.symbol.clone(),
        side: convert_virs_position_side(&ep.side),
        size: ep.size,
        entry_price: ep.entry_price,
        leverage: ep.leverage,
        unrealized_pnl: ep.unrealized_pnl,
        liquidation_price: ep.liquidation_price,
    }
}

pub(crate) fn to_pe_error(e: anyhow::Error) -> crate::engine::position::error::PositionEngineError {
    crate::engine::position::error::PositionEngineError::Exchange(e.to_string())
}

/// 将 VIRS Ticker 转换为 PE Ticker
pub(crate) fn convert_ticker(t: &models::Ticker) -> Ticker {
    Ticker {
        symbol: t.symbol.clone(),
        price: t.last,
        bid: t.bid,
        ask: t.ask,
        volume_24h: t.volume_24h,
        timestamp: t.timestamp,
    }
}

/// 将 VIRS FundingRate 转换为 PE FundingRate
pub(crate) fn convert_funding_rate(fr: &models::FundingRate) -> FundingRate {
    FundingRate {
        symbol: fr.symbol.clone(),
        rate: fr.rate,
        next_funding_time: fr.next_funding_time.unwrap_or_else(Utc::now),
    }
}

// ── Exchange trait 实现 ──

#[async_trait]
impl PeExchange for CcxtExchangeAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn market_type(&self) -> MarketType {
        convert_virs_market_type(&self.inner.market_type())
    }

    // ── 行情数据 ──

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker> {
        let t = self.inner.get_ticker(symbol).await.map_err(to_pe_error)?;
        Ok(convert_ticker(&t))
    }

    async fn get_balance(&self) -> Result<Balance> {
        let balances = self.inner.get_balances().await.map_err(to_pe_error)?;
        let usdt = balances.iter().find(|b| b.asset.eq_ignore_ascii_case("USDT"));
        Ok(Balance {
            asset: "USDT".to_string(),
            free: usdt.map(|b| b.free).unwrap_or(0.0),
            used: usdt.map(|b| b.used).unwrap_or(0.0),
            total: usdt.map(|b| b.total).unwrap_or(0.0),
        })
    }

    async fn get_positions(&self, symbol: Option<&str>) -> Result<Vec<ExchangePosition>> {
        let positions = self.inner.get_positions(symbol).await.map_err(to_pe_error)?;
        Ok(positions.iter().map(convert_exchange_position).collect())
    }

    async fn get_funding_rate(&self, symbol: &str) -> Result<FundingRate> {
        let fr = self.inner.get_funding_rate(symbol).await.map_err(to_pe_error)?;
        Ok(convert_funding_rate(&fr))
    }

    async fn get_fee_rates(&self, symbol: &str) -> Result<FeeRates> {
        warn!(symbol, "get_fee_rates not implemented in VIRS Exchange, using defaults");
        Ok(FeeRates {
            symbol: symbol.to_string(),
            maker_rate: 0.0002,
            taker_rate: 0.0005,
        })
    }

    // ── 交易 ──

    async fn place_order(&self, params: PlaceOrderParams) -> Result<Order> {
        let virs_order = self.inner.place_order_with_options(
            &params.symbol,
            convert_to_virs_side(&params.side),
            convert_order_type(&params.order_type),
            params.amount,
            params.price,
            if params.reduce_only { Some(true) } else { None },
            convert_position_side(&params.position_side),
        ).await.map_err(to_pe_error)?;

        let mut pe_order = convert_order(&virs_order);
        pe_order.exchange = self.name().to_string();
        Ok(pe_order)
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order> {
        let virs_order = self.inner.cancel_order(symbol, order_id).await.map_err(to_pe_error)?;
        let mut pe_order = convert_order(&virs_order);
        pe_order.exchange = self.name().to_string();
        Ok(pe_order)
    }

    async fn cancel_all_orders(&self, symbol: &str) -> Result<Vec<Order>> {
        let open_orders = self.inner.get_open_orders(Some(symbol)).await.map_err(to_pe_error)?;

        let mut canceled = Vec::new();
        for o in &open_orders {
            match self.inner.cancel_order(symbol, &o.id).await {
                Ok(virs_order) => {
                    let mut pe_order = convert_order(&virs_order);
                    pe_order.exchange = self.name().to_string();
                    canceled.push(pe_order);
                }
                Err(e) => {
                    warn!(order_id = %o.id, error = %e, "Failed to cancel order");
                }
            }
        }
        Ok(canceled)
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>> {
        let orders = self.inner.get_open_orders(symbol).await.map_err(to_pe_error)?;
        Ok(orders.iter().map(|o| {
            let mut pe = convert_order(o);
            pe.exchange = self.name().to_string();
            pe
        }).collect())
    }

    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order> {
        let virs_order = self.inner.get_order(symbol, order_id).await.map_err(to_pe_error)?;
        let mut pe_order = convert_order(&virs_order);
        pe_order.exchange = self.name().to_string();
        Ok(pe_order)
    }

    // ── 永续合约 ──

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<()> {
        self.inner.set_leverage(symbol, leverage).await.map_err(to_pe_error)
    }

    // ── WebSocket 成交回报 ──

    async fn subscribe_order_updates(&self, symbols: &[&str]) -> Result<mpsc::Receiver<WsFeedEvent>> {
        let (tx, rx) = mpsc::channel(256);

        if let Some(ref listen_key) = self.listen_key {
            // 有 listenKey，启动真正的 WS 连接
            let is_perpetual = self.inner.market_type() == models::MarketType::Perpetual;
            let mut ws = if is_perpetual {
                BinanceOrderWs::new_perpetual(listen_key.clone())
            } else {
                BinanceOrderWs::new_spot(listen_key.clone())
            };

            info!(
                exchange = %self.inner.name(),
                market = ?self.inner.market_type(),
                symbols_count = symbols.len(),
                "Starting WebSocket order updates"
            );

            ws.start(tx).await;
        } else {
            // 无 listenKey，退化为轮询模式
            drop(tx);
            warn!("No listenKey configured, WebSocket order updates disabled, using polling mode only");
        }

        Ok(rx)
    }
}
