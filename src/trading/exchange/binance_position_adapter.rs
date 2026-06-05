//! CcxtExchangeAdapter — 将 VIRS ExchangeRegistry 适配为 Position Engine 的 Exchange trait
//!
//! 持有 Arc<ExchangeRegistry> 引用，在调用时动态查找已注册的交易所。
//! 支持交易所动态注册（通过 API），无需在启动时确定交易所实例。

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::mpsc;
use tracing::{warn, info};

use crate::trading::exchange::registry::ExchangeRegistry;
use crate::engine::position::exchange::Exchange as PeExchange;
use crate::engine::position::types::*;
use crate::engine::position::error::{PositionEngineError, Result};

/// 适配器：将 VIRS ExchangeRegistry 适配为 Position Engine 的 Exchange trait
///
/// 持有 Arc<ExchangeRegistry> 引用，在调用时动态查找已注册的交易所。
/// 支持交易所动态注册（通过 API），无需在启动时确定交易所实例。
pub struct CcxtExchangeAdapter {
    registry: Arc<ExchangeRegistry>,
    /// 缓存的交易所名称
    cached_name: String,
    /// 可选的 WS listenKey，用于启动订单推送
    listen_key: Option<String>,
}

impl CcxtExchangeAdapter {
    pub fn new(registry: Arc<ExchangeRegistry>) -> Self {
        Self {
            registry,
            cached_name: "binance".to_string(),
            listen_key: None,
        }
    }

    /// 设置 listenKey 以启用 WebSocket 订单推送
    pub fn with_listen_key(mut self, listen_key: String) -> Self {
        self.listen_key = Some(listen_key);
        self
    }

    /// 设置交易所名称
    pub fn with_name(mut self, name: String) -> Self {
        self.cached_name = name;
        self
    }

    /// 获取任意已注册的 perpetual exchange
    fn get_perpetual_exchange(&self) -> Option<dashmap::mapref::one::Ref<'_, String, Box<dyn crate::trading::exchange::Exchange>>> {
        for entry in self.registry.registered_names() {
            if entry.contains("perpetual") {
                if let Some(ex) = self.registry.get(&entry) {
                    return Some(ex);
                }
            }
        }
        None
    }
}

// ── 类型转换辅助函数 ──

pub(crate) fn convert_side(side: &crate::models::Side) -> Side {
    match side {
        crate::models::Side::Buy => Side::Buy,
        crate::models::Side::Sell => Side::Sell,
    }
}

pub(crate) fn convert_to_virs_side(side: &Side) -> crate::models::Side {
    match side {
        Side::Buy => crate::models::Side::Buy,
        Side::Sell => crate::models::Side::Sell,
    }
}

pub(crate) fn convert_position_side(side: &Option<PositionSide>) -> Option<crate::models::PositionSide> {
    side.as_ref().map(|s| match s {
        PositionSide::Long => crate::models::PositionSide::Long,
        PositionSide::Short => crate::models::PositionSide::Short,
        PositionSide::Both => crate::models::PositionSide::Long,
    })
}

pub(crate) fn convert_order_type(ot: &OrderType) -> crate::models::OrderType {
    match ot {
        OrderType::Limit => crate::models::OrderType::Limit,
        OrderType::Market => crate::models::OrderType::Market,
        OrderType::StopMarket => crate::models::OrderType::StopMarket,
        OrderType::TakeProfitMarket => crate::models::OrderType::StopMarket,
    }
}

pub(crate) fn convert_order_status(status: &crate::models::OrderStatus) -> OrderStatus {
    match status {
        crate::models::OrderStatus::Pending => OrderStatus::Pending,
        crate::models::OrderStatus::Open => OrderStatus::Open,
        crate::models::OrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
        crate::models::OrderStatus::Filled => OrderStatus::Filled,
        crate::models::OrderStatus::Canceled => OrderStatus::Canceled,
        crate::models::OrderStatus::Failed => OrderStatus::Failed,
    }
}

pub(crate) fn convert_virs_position_side(side: &crate::models::PositionSide) -> PositionSide {
    match side {
        crate::models::PositionSide::Long => PositionSide::Long,
        crate::models::PositionSide::Short => PositionSide::Short,
    }
}

pub(crate) fn convert_virs_market_type(mt: &crate::models::MarketType) -> MarketType {
    match mt {
        crate::models::MarketType::Spot => MarketType::Spot,
        crate::models::MarketType::Perpetual => MarketType::Perpetual,
    }
}

pub(crate) fn convert_order(o: &crate::models::Order, exchange_name: &str) -> Order {
    Order {
        id: uuid::Uuid::parse_str(&o.id).unwrap_or_else(|_| uuid::Uuid::new_v4()),
        position_id: uuid::Uuid::nil(),
        exchange_order_id: Some(o.id.clone()),
        client_order_id: o.client_order_id.clone(),
        exchange: exchange_name.to_string(),
        symbol: o.symbol.clone(),
        side: convert_side(&o.side),
        order_type: match o.order_type {
            crate::models::OrderType::Limit => OrderType::Limit,
            crate::models::OrderType::Market => OrderType::Market,
            crate::models::OrderType::StopMarket => OrderType::StopMarket,
            crate::models::OrderType::StopLimit => OrderType::StopMarket,
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

pub(crate) fn convert_exchange_position(ep: &crate::models::ExchangePosition) -> ExchangePosition {
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

pub(crate) fn to_pe_error(e: anyhow::Error) -> PositionEngineError {
    PositionEngineError::Exchange(e.to_string())
}

pub(crate) fn no_exchange_error() -> PositionEngineError {
    PositionEngineError::Exchange("No perpetual exchange registered in ExchangeRegistry".to_string())
}

/// 将 VIRS Ticker 转换为 PE Ticker
#[cfg(test)]
pub(crate) fn convert_ticker(t: &crate::models::Ticker) -> Ticker {
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
#[cfg(test)]
pub(crate) fn convert_funding_rate(fr: &crate::models::FundingRate) -> FundingRate {
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
        &self.cached_name
    }

    fn market_type(&self) -> MarketType {
        if let Some(ex) = self.get_perpetual_exchange() {
            convert_virs_market_type(&ex.market_type())
        } else {
            MarketType::Perpetual
        }
    }

    // ── 行情数据 ──

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let t = ex.get_ticker(symbol).await.map_err(to_pe_error)?;
        Ok(Ticker {
            symbol: t.symbol.clone(),
            price: t.last,
            bid: t.bid,
            ask: t.ask,
            volume_24h: t.volume_24h,
            timestamp: t.timestamp,
        })
    }

    async fn get_balance(&self) -> Result<Balance> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let balances = ex.get_balances().await.map_err(to_pe_error)?;
        let usdt = balances.iter().find(|b| b.asset.eq_ignore_ascii_case("USDT"));
        Ok(Balance {
            asset: "USDT".to_string(),
            free: usdt.map(|b| b.free).unwrap_or(0.0),
            used: usdt.map(|b| b.used).unwrap_or(0.0),
            total: usdt.map(|b| b.total).unwrap_or(0.0),
        })
    }

    async fn get_positions(&self, symbol: Option<&str>) -> Result<Vec<ExchangePosition>> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let positions = ex.get_positions(symbol).await.map_err(to_pe_error)?;
        Ok(positions.iter().map(convert_exchange_position).collect())
    }

    async fn get_funding_rate(&self, symbol: &str) -> Result<FundingRate> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let fr = ex.get_funding_rate(symbol).await.map_err(to_pe_error)?;
        Ok(FundingRate {
            symbol: fr.symbol.clone(),
            rate: fr.rate,
            next_funding_time: fr.next_funding_time.unwrap_or_else(Utc::now),
        })
    }

    async fn get_fee_rates(&self, symbol: &str) -> Result<FeeRates> {
        let _ = symbol;
        // VIRS Exchange 没有 get_fee_rates 方法，使用默认值
        Ok(FeeRates {
            symbol: symbol.to_string(),
            maker_rate: 0.0002,
            taker_rate: 0.0005,
        })
    }

    // ── 交易 ──

    async fn place_order(&self, params: PlaceOrderParams) -> Result<Order> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let exchange_name = ex.name().to_string();

        let reduce_only_param = if params.position_side.is_some() {
            None
        } else if params.reduce_only {
            Some(true)
        } else {
            None
        };

        let virs_order = ex.place_order_with_options(
            &params.symbol,
            convert_to_virs_side(&params.side),
            convert_order_type(&params.order_type),
            params.amount,
            params.price,
            reduce_only_param,
            convert_position_side(&params.position_side),
        ).await.map_err(to_pe_error)?;

        Ok(convert_order(&virs_order, &exchange_name))
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let exchange_name = ex.name().to_string();
        let virs_order = ex.cancel_order(symbol, order_id).await.map_err(to_pe_error)?;
        Ok(convert_order(&virs_order, &exchange_name))
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let exchange_name = ex.name().to_string();
        let open_orders = ex.get_open_orders(symbol).await.map_err(to_pe_error)?;

        let mut canceled = Vec::new();
        for o in &open_orders {
            let sym = o.symbol.as_str();
            match ex.cancel_order(sym, &o.id).await {
                Ok(virs_order) => {
                    canceled.push(convert_order(&virs_order, &exchange_name));
                }
                Err(e) => {
                    warn!(order_id = %o.id, error = %e, "Failed to cancel order");
                }
            }
        }
        Ok(canceled)
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let exchange_name = ex.name().to_string();
        let orders = ex.get_open_orders(symbol).await.map_err(to_pe_error)?;
        Ok(orders.iter().map(|o| convert_order(o, &exchange_name)).collect())
    }

    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let exchange_name = ex.name().to_string();
        let virs_order = ex.get_order(symbol, order_id).await.map_err(to_pe_error)?;
        Ok(convert_order(&virs_order, &exchange_name))
    }

    // ── 永续合约 ──

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<()> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        ex.set_leverage(symbol, leverage).await.map_err(to_pe_error)
    }

    async fn get_position_mode(&self) -> Result<PositionMode> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let mode = ex.get_position_mode().await.map_err(to_pe_error)?;
        Ok(match mode {
            crate::models::PositionMode::OneWay => PositionMode::OneWay,
            crate::models::PositionMode::Hedge => PositionMode::Hedge,
        })
    }

    // ── WebSocket 成交回报 ──

    async fn subscribe_order_updates(&self, symbols: &[&str]) -> Result<mpsc::Receiver<WsFeedEvent>> {
        let (tx, rx) = mpsc::channel(256);

        if let Some(ref listen_key) = self.listen_key {
            let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
            let is_perpetual = ex.market_type() == crate::models::MarketType::Perpetual;
            let mut ws = if is_perpetual {
                crate::trading::ccxt::adapter::binance::order_ws::BinanceOrderWs::new_perpetual(listen_key.clone())
            } else {
                crate::trading::ccxt::adapter::binance::order_ws::BinanceOrderWs::new_spot(listen_key.clone())
            };

            info!(
                symbols_count = symbols.len(),
                "Starting WebSocket order updates via CcxtExchangeAdapter"
            );

            ws.start(tx).await;
        } else {
            // 无 listenKey，尝试从交易所动态获取
            let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
            match ex.create_listen_key().await {
                Ok(key) => {
                    let is_perpetual = ex.market_type() == crate::models::MarketType::Perpetual;
                    let mut ws = if is_perpetual {
                        crate::trading::ccxt::adapter::binance::order_ws::BinanceOrderWs::new_perpetual(key)
                    } else {
                        crate::trading::ccxt::adapter::binance::order_ws::BinanceOrderWs::new_spot(key)
                    };

                    info!("Obtained listenKey dynamically, starting WebSocket order updates");
                    ws.start(tx).await;
                }
                Err(e) => {
                    drop(tx);
                    warn!(error = %e, "No listenKey and failed to create one, WebSocket order updates disabled");
                }
            }
        }

        Ok(rx)
    }
}
