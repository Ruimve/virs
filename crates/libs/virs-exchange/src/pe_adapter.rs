//! CcxtExchangeAdapter — adapts ExchangeRegistry to Position Engine's ExchangePe trait.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{warn, info};

use virs_types::enums::*;
use virs_types::market::*;
use virs_types::position::*;
use virs_types::exchange_pe::{ExchangePe, OrderUpdateStream};
use virs_models as models;

use crate::Exchange;
use crate::registry::ExchangeRegistry;

/// Adapter: ExchangeRegistry → Position Engine ExchangePe trait
pub struct CcxtExchangeAdapter {
    registry: Arc<ExchangeRegistry>,
    cached_name: String,
    listen_key: Option<String>,
}

impl CcxtExchangeAdapter {
    pub fn new(registry: Arc<ExchangeRegistry>) -> Self {
        Self { registry, cached_name: "binance".to_string(), listen_key: None }
    }

    pub fn with_listen_key(mut self, listen_key: String) -> Self {
        self.listen_key = Some(listen_key);
        self
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.cached_name = name;
        self
    }

    fn get_perpetual_exchange(&self) -> Option<dashmap::mapref::one::Ref<'_, String, Box<dyn Exchange>>> {
        for entry in self.registry.registered_names() {
            if entry.contains("perpetual") {
                if let Some(ex) = self.registry.get(&entry) { return Some(ex); }
            }
        }
        None
    }
}

// ---- Type conversion helpers ----

fn convert_side(side: &models::Side) -> Side {
    match side {
        models::Side::Buy => Side::Buy,
        models::Side::Sell => Side::Sell,
    }
}

fn convert_to_models_side(side: &Side) -> models::Side {
    match side {
        Side::Buy => models::Side::Buy,
        Side::Sell => models::Side::Sell,
    }
}

fn convert_position_side(side: &Option<PositionSide>) -> Option<models::PositionSide> {
    side.as_ref().map(|s| match s {
        PositionSide::Long => models::PositionSide::Long,
        PositionSide::Short => models::PositionSide::Short,
        PositionSide::Both => models::PositionSide::Long,
    })
}

fn convert_order_type(ot: &OrderType) -> models::OrderType {
    match ot {
        OrderType::Limit => models::OrderType::Limit,
        OrderType::Market => models::OrderType::Market,
        OrderType::StopMarket => models::OrderType::StopMarket,
        OrderType::TakeProfitMarket => models::OrderType::StopMarket,
        OrderType::StopLimit => models::OrderType::StopLimit,
    }
}

fn convert_order_status(status: &models::OrderStatus) -> OrderStatus {
    match status {
        models::OrderStatus::Pending => OrderStatus::Pending,
        models::OrderStatus::Open => OrderStatus::Open,
        models::OrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
        models::OrderStatus::Filled => OrderStatus::Filled,
        models::OrderStatus::Canceled => OrderStatus::Canceled,
        models::OrderStatus::Failed => OrderStatus::Failed,
    }
}

fn convert_virs_position_side(side: &models::PositionSide) -> PositionSide {
    match side {
        models::PositionSide::Long => PositionSide::Long,
        models::PositionSide::Short => PositionSide::Short,
        models::PositionSide::Both => PositionSide::Both,
    }
}

fn convert_virs_market_type(mt: &models::MarketType) -> MarketType {
    match mt {
        models::MarketType::Spot => MarketType::Spot,
        models::MarketType::Perpetual => MarketType::Perpetual,
    }
}

fn convert_order(o: &models::Order, exchange_name: &str) -> PositionOrder {
    PositionOrder {
        id: uuid::Uuid::parse_str(&o.id).unwrap_or_else(|_| uuid::Uuid::new_v4()),
        position_id: uuid::Uuid::nil(),
        exchange_order_id: Some(o.id.clone()),
        client_order_id: o.client_order_id.clone(),
        exchange: exchange_name.to_string(),
        symbol: o.symbol.clone(),
        side: convert_side(&o.side),
        order_type: match o.order_type {
            models::OrderType::Limit => OrderType::Limit,
            models::OrderType::Market => OrderType::Market,
            models::OrderType::StopMarket => OrderType::StopMarket,
            models::OrderType::StopLimit => OrderType::StopLimit,
            models::OrderType::TakeProfitMarket => OrderType::TakeProfitMarket,
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

fn convert_exchange_position(ep: &models::ExchangePosition) -> ExchangePosition {
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

fn to_pe_error(e: anyhow::Error) -> PositionEngineError {
    PositionEngineError::Exchange(e.to_string())
}

fn no_exchange_error() -> PositionEngineError {
    PositionEngineError::Exchange("No perpetual exchange registered in ExchangeRegistry".to_string())
}

/// Convert ccxt WsFeedEvent to virs_types WsFeedEvent
fn convert_ws_feed_event(event: virs_ccxt::ws_types::WsFeedEvent) -> WsFeedEvent {
    match event {
        virs_ccxt::ws_types::WsFeedEvent::OrderUpdate {
            exchange_order_id, symbol, status, filled, remaining, price, amount, commission, timestamp, position_side,
        } => WsFeedEvent::OrderUpdate {
            exchange_order_id, symbol,
            status: match status {
                virs_types::OrderStatus::Pending => OrderStatus::Pending,
                virs_types::OrderStatus::Open => OrderStatus::Open,
                virs_types::OrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
                virs_types::OrderStatus::Filled => OrderStatus::Filled,
                virs_types::OrderStatus::Canceled => OrderStatus::Canceled,
                virs_types::OrderStatus::Failed => OrderStatus::Failed,
            },
            filled, remaining, price, amount, commission, timestamp,
            position_side: position_side.map(|ps| match ps {
                virs_types::PositionSide::Long => PositionSide::Long,
                virs_types::PositionSide::Short => PositionSide::Short,
                virs_types::PositionSide::Both => PositionSide::Both,
            }),
        },
        virs_ccxt::ws_types::WsFeedEvent::ConnectionChanged { connected } => WsFeedEvent::ConnectionChanged { connected },
    }
}

#[async_trait]
impl ExchangePe for CcxtExchangeAdapter {
    fn name(&self) -> &str { &self.cached_name }

    fn market_type(&self) -> MarketType {
        if let Some(ex) = self.get_perpetual_exchange() {
            convert_virs_market_type(&ex.market_type())
        } else {
            MarketType::Perpetual
        }
    }

    async fn get_ticker(&self, symbol: &str) -> PositionResult<Ticker> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let t = ex.get_ticker(symbol).await.map_err(to_pe_error)?;
        Ok(Ticker {
            symbol: t.symbol.clone(), exchange: t.exchange, bid: t.bid, ask: t.ask, last: t.last,
            high_24h: t.high_24h, low_24h: t.low_24h, volume_24h: t.volume_24h,
            price_change_24h: t.price_change_24h, price_change_pct_24h: t.price_change_pct_24h,
            timestamp: t.timestamp,
        })
    }

    async fn get_balance(&self) -> PositionResult<Balance> {
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

    async fn get_positions(&self, symbol: Option<&str>) -> PositionResult<Vec<ExchangePosition>> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let positions = ex.get_positions(symbol).await.map_err(to_pe_error)?;
        Ok(positions.iter().map(convert_exchange_position).collect())
    }

    async fn get_funding_rate(&self, symbol: &str) -> PositionResult<FundingRate> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let fr = ex.get_funding_rate(symbol).await.map_err(to_pe_error)?;
        Ok(FundingRate { symbol: fr.symbol, rate: fr.rate, next_funding_time: fr.next_funding_time })
    }

    async fn get_fee_rates(&self, _symbol: &str) -> PositionResult<FeeRates> {
        Ok(FeeRates { symbol: _symbol.to_string(), maker_rate: 0.0002, taker_rate: 0.0005 })
    }

    async fn place_order(&self, params: PlaceOrderParams) -> PositionResult<PositionOrder> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let exchange_name = ex.name().to_string();
        let reduce_only_param = if params.position_side.is_some() { None } else if params.reduce_only { Some(true) } else { None };
        let virs_order = ex.place_order_with_options(
            &params.symbol, convert_to_models_side(&params.side), convert_order_type(&params.order_type),
            params.amount, params.price, reduce_only_param, convert_position_side(&params.position_side),
        ).await.map_err(to_pe_error)?;
        Ok(convert_order(&virs_order, &exchange_name))
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> PositionResult<PositionOrder> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let exchange_name = ex.name().to_string();
        let virs_order = ex.cancel_order(symbol, order_id).await.map_err(to_pe_error)?;
        Ok(convert_order(&virs_order, &exchange_name))
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> PositionResult<Vec<PositionOrder>> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let exchange_name = ex.name().to_string();
        let open_orders = ex.get_open_orders(symbol).await.map_err(to_pe_error)?;
        let mut canceled = Vec::new();
        for o in &open_orders {
            match ex.cancel_order(&o.symbol, &o.id).await {
                Ok(virs_order) => canceled.push(convert_order(&virs_order, &exchange_name)),
                Err(e) => warn!(order_id = %o.id, error = %e, "Failed to cancel order"),
            }
        }
        Ok(canceled)
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> PositionResult<Vec<PositionOrder>> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let exchange_name = ex.name().to_string();
        let orders = ex.get_open_orders(symbol).await.map_err(to_pe_error)?;
        Ok(orders.iter().map(|o| convert_order(o, &exchange_name)).collect())
    }

    async fn get_order(&self, symbol: &str, order_id: &str) -> PositionResult<PositionOrder> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let exchange_name = ex.name().to_string();
        let virs_order = ex.get_order(symbol, order_id).await.map_err(to_pe_error)?;
        Ok(convert_order(&virs_order, &exchange_name))
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> PositionResult<()> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        ex.set_leverage(symbol, leverage).await.map_err(to_pe_error)
    }

    async fn get_position_mode(&self) -> PositionResult<PositionMode> {
        let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
        let mode = ex.get_position_mode().await.map_err(to_pe_error)?;
        Ok(match mode {
            models::PositionMode::OneWay => PositionMode::OneWay,
            models::PositionMode::Hedge => PositionMode::Hedge,
        })
    }

    async fn subscribe_order_updates(&self, symbols: &[&str]) -> PositionResult<OrderUpdateStream> {
        let (tx, rx) = mpsc::channel(256);

        // Spawn a task that receives ccxt WsFeedEvents and converts them
        let (ccxt_tx, ccxt_rx) = mpsc::channel(256);

        if let Some(ref listen_key) = self.listen_key {
            let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
            let is_perpetual = ex.market_type() == models::MarketType::Perpetual;
            let mut ws = if is_perpetual {
                virs_ccxt::adapter::binance::order_ws::BinanceOrderWs::new_perpetual(listen_key.clone())
            } else {
                virs_ccxt::adapter::binance::order_ws::BinanceOrderWs::new_spot(listen_key.clone())
            };
            info!(symbols_count = symbols.len(), "Starting WebSocket order updates via CcxtExchangeAdapter");
            ws.start(ccxt_tx).await;
        } else {
            let ex = self.get_perpetual_exchange().ok_or_else(no_exchange_error)?;
            match ex.create_listen_key().await {
                Ok(key) => {
                    let is_perpetual = ex.market_type() == models::MarketType::Perpetual;
                    let mut ws = if is_perpetual {
                        virs_ccxt::adapter::binance::order_ws::BinanceOrderWs::new_perpetual(key)
                    } else {
                        virs_ccxt::adapter::binance::order_ws::BinanceOrderWs::new_spot(key)
                    };
                    info!("Obtained listenKey dynamically, starting WebSocket order updates");
                    ws.start(ccxt_tx).await;
                }
                Err(e) => {
                    drop(ccxt_tx);
                    drop(tx);
                    warn!(error = %e, "No listenKey and failed to create one, WebSocket order updates disabled");
                    return Err(no_exchange_error());
                }
            }
        }

        // Spawn converter task
        tokio::spawn(async move {
            let mut ccxt_rx = ccxt_rx;
            while let Some(event) = ccxt_rx.recv().await {
                let converted = convert_ws_feed_event(event);
                if tx.send(converted).await.is_err() {
                    break;
                }
            }
        });

        // Convert mpsc::Receiver to Stream via ReceiverStream
        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
