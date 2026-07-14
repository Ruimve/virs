use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tracing::warn;
use uuid::Uuid;

use virs_types::enums::*;
use virs_types::exchange_pe::{ExchangePe, OrderUpdateStream};
use virs_types::market::*;
use virs_types::position::*;

use virs_error::{ExchangeError, VirsError, VirsResult};

use crate::registry::Exchanges;


#[derive(Debug, Clone)]
struct PaperPendingOrder {
    id: Uuid,
    symbol: String,
    side: Side,
    order_type: OrderType,
    amount: f64,
    price: Option<f64>,
    reduce_only: bool,
    position_side: Option<PositionSide>,
    client_order_id: Option<String>,
    created_at: chrono::DateTime<Utc>,
}


type PaperPosition = ExchangePosition;


pub struct PaperExchangeAdapter {
    name: String,
    market_type: MarketType,
    pending: Arc<DashMap<Uuid, PaperPendingOrder>>,
    positions: Arc<DashMap<String, PaperPosition>>,
    balance: Arc<Mutex<Balance>>,
    price_tx: Arc<Mutex<Option<mpsc::Sender<WsFeedEvent>>>>,
    last_prices: Arc<DashMap<String, f64>>,
    exchange_registry: Option<Arc<Exchanges>>,
    balance_initialized: Arc<AtomicBool>,

    configured_leverage: Arc<DashMap<String, u32>>,
}


pub fn compute_paper_liquidation_price(
    entry_price: f64,
    side: PositionSide,
    leverage: u32,
) -> Option<f64> {
    if leverage == 0 || entry_price <= 0.0 {
        return None;
    }
    let ratio = 1.0 / leverage as f64;
    Some(match side {
        PositionSide::Long => entry_price * (1.0 - ratio),
        PositionSide::Short => entry_price * (1.0 + ratio),
    })
}

impl PaperExchangeAdapter {
    pub fn new(name: &str, market_type: MarketType, initial_balance: f64) -> Self {
        Self {
            name: name.to_string(),
            market_type,
            pending: Arc::new(DashMap::new()),
            positions: Arc::new(DashMap::new()),
            balance: Arc::new(Mutex::new(Balance {
                asset: "USDT".to_string(),
                free: initial_balance,
                used: 0.0,
                total: initial_balance,
            })),
            price_tx: Arc::new(Mutex::new(None)),
            last_prices: Arc::new(DashMap::new()),
            exchange_registry: None,
            balance_initialized: Arc::new(AtomicBool::new(initial_balance > 0.0)),
            configured_leverage: Arc::new(DashMap::new()),
        }
    }

    pub fn with_exchange_registry(mut self, registry: Arc<Exchanges>) -> Self {
        self.exchange_registry = Some(registry);
        self
    }

    async fn ensure_balance_initialized(&self) {
        if self.balance_initialized.load(Ordering::Relaxed) {
            return;
        }
        let registry = match &self.exchange_registry {
            Some(r) => r.clone(),
            None => return,
        };
        let exchange = registry
            .registered_names()
            .iter()
            .find(|n| n.contains("perpetual"))
            .and_then(|key| registry.get(key));
        if let Some(ex) = exchange {
            match ex.get_balances().await {
                Ok(balances) => {
                    if let Some(usdt) = balances
                        .iter()
                        .find(|b| b.asset.eq_ignore_ascii_case("USDT"))
                    {
                        let mut balance = self.balance.lock().await;
                        if !self.balance_initialized.load(Ordering::Relaxed) {
                            balance.free = usdt.free;
                            balance.used = usdt.used;
                            balance.total = usdt.total;
                            self.balance_initialized.store(true, Ordering::Relaxed);
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "PaperExchangeAdapter: failed to fetch balance from real exchange");
                }
            }
        }
    }

    pub async fn on_price_tick(&self, symbol: &str, current_price: f64) {
        if current_price <= 0.0 {
            return;
        }
        self.last_prices.insert(symbol.to_string(), current_price);

        let mut triggered = Vec::new();
        for entry in self.pending.iter() {
            let order = entry.value();
            if order.symbol != symbol {
                continue;
            }
            let filled = match order.side {
                Side::Buy => current_price <= order.price.unwrap_or(current_price),
                Side::Sell => current_price >= order.price.unwrap_or(current_price),
            };
            if filled {
                triggered.push(order.clone());
            }
        }

        for order in &triggered {
            self.pending.remove(&order.id);
            self.update_position_on_fill(order, current_price).await;

            let fee = current_price * order.amount * 0.0002;
            let tx = self.price_tx.lock().await;
            if let Some(ref tx) = *tx {
                if tx
                    .send(WsFeedEvent::OrderUpdate {
                        exchange_order_id: order.id.to_string(),
                        client_order_id: order.client_order_id.clone(),
                        symbol: order.symbol.clone(),
                        status: OrderStatus::Filled,
                        filled: order.amount,
                        remaining: 0.0,
                        price: current_price,
                        amount: order.amount,
                        commission: fee,
                        timestamp: Utc::now(),
                        position_side: order.position_side,
                    })
                    .await
                    .is_err()
                {
                    warn!(order_id = %order.id, symbol = %order.symbol, "Paper WsFeedEvent::OrderUpdate send failed — receiver dropped, event lost");
                }
            }
        }
        self.update_unrealized_pnl(symbol, current_price).await;
    }

    async fn update_position_on_fill(&self, order: &PaperPendingOrder, fill_price: f64) {

        let position_side = match order.position_side {
            Some(ps) => ps,
            None => {
                tracing::error!(
                    symbol = %order.symbol,
                    order_id = %order.id,
                    "position_side is None in Hedge mode — skipping fill update to avoid \
                     silent position corruption. Caller must provide position_side."
                );
                return;
            }
        };
        let key = format!("{}:{:?}", order.symbol, position_side);


        let size_delta = order.amount;


        let leverage: u32 = self
            .configured_leverage
            .get(&order.symbol)
            .map(|v| *v)
            .unwrap_or_else(|| {
                tracing::error!(
                    symbol = %order.symbol,
                    "No leverage configured for symbol — defaulting to 1 (no leverage). \
                     Call set_leverage() before trading to avoid unexpected margin calculations."
                );
                1
            });
        let leverage_f64 = leverage as f64;
        let notional = fill_price * order.amount;
        let margin = notional / leverage_f64;

        let is_opening = match (order.side, position_side) {
            (Side::Buy, PositionSide::Long) => true,
            (Side::Sell, PositionSide::Short) => true,
            (Side::Sell, PositionSide::Long) => false,
            (Side::Buy, PositionSide::Short) => false,
        };


        let old_pos_info = self
            .positions
            .get(&key)
            .map(|p| (p.side, p.entry_price, p.size));


        let realized_pnl: f64 = match (&old_pos_info, is_opening) {
            (Some((side, entry, old_size)), false) => {
                let closed = order.amount.min(*old_size);
                match side {
                    PositionSide::Long => (fill_price - entry) * closed,
                    PositionSide::Short => (entry - fill_price) * closed,
                }
            }
            _ => 0.0,
        };


        match self.positions.get_mut(&key) {
            Some(mut pos) => {
                if is_opening {

                    let old_size = pos.size;
                    let new_size = old_size + size_delta;
                    let total_cost = pos.entry_price * old_size + fill_price * size_delta;
                    pos.size = new_size;
                    pos.entry_price = total_cost / new_size;
                    pos.leverage = leverage;
                    pos.liquidation_price =
                        compute_paper_liquidation_price(pos.entry_price, pos.side, leverage);
                } else {

                    let new_size = pos.size - size_delta;
                    if new_size < 1e-8 {

                        drop(pos);
                        self.positions.remove(&key);
                    } else {

                        pos.size = new_size;
                        pos.leverage = leverage;
                        pos.liquidation_price =
                            compute_paper_liquidation_price(pos.entry_price, pos.side, leverage);
                    }
                }
            }
            None => {

                let liq_price = compute_paper_liquidation_price(fill_price, position_side, leverage);
                self.positions.insert(
                    key.clone(),
                    PaperPosition {
                        symbol: order.symbol.clone(),
                        side: position_side,
                        size: size_delta,
                        entry_price: fill_price,
                        leverage,
                        unrealized_pnl: 0.0,
                        liquidation_price: liq_price,
                    },
                );
            }
        }


        let mut balance = self.balance.lock().await;

        if is_opening {
            balance.used += margin;
            balance.free -= margin;
        } else {

            let margin_release = margin.min(balance.used);
            balance.used -= margin_release;
            balance.free += margin_release + realized_pnl;
        }
    }

    async fn update_unrealized_pnl(&self, symbol: &str, current_price: f64) {
        for mut entry in self.positions.iter_mut() {
            let pos = entry.value_mut();
            if pos.symbol != symbol {
                continue;
            }
            pos.unrealized_pnl = pos.unrealized_pnl_at(current_price);
        }
    }
}

#[async_trait]
impl ExchangePe for PaperExchangeAdapter {
    fn name(&self) -> &str {
        &self.name
    }
    fn market_type(&self) -> MarketType {
        self.market_type
    }

    async fn get_ticker(&self, symbol: &str) -> VirsResult<Ticker> {
        warn!(
            exchange = %self.name,
            symbol = %symbol,
            "PaperExchange get_ticker stub — returning all-zero Ticker (paper mode, no real market data)"
        );
        Ok(Ticker {
            symbol: symbol.to_string(),
            exchange: self.name.clone(),
            bid: None,
            ask: None,
            last: 0.0,
            high_24h: 0.0,
            low_24h: 0.0,
            volume_24h: 0.0,
            price_change_24h: 0.0,
            price_change_pct_24h: 0.0,
            timestamp: Utc::now(),
        })
    }

    async fn get_balance(&self) -> VirsResult<Balance> {
        self.ensure_balance_initialized().await;
        let mut balance = self.balance.lock().await;
        balance.total = balance.compute_total();
        Ok(balance.clone())
    }

    async fn get_positions(&self, symbol: Option<&str>) -> VirsResult<Vec<ExchangePosition>> {
        Ok(self
            .positions
            .iter()
            .filter(|e| {
                let pos = e.value();
                symbol.is_none_or(|s| pos.symbol == s) && pos.size.abs() > 1e-8
            })
            .map(|e| {
                let pos = e.value();
                ExchangePosition {
                    symbol: pos.symbol.clone(),
                    side: pos.side,
                    size: pos.size,
                    entry_price: pos.entry_price,
                    leverage: pos.leverage,
                    unrealized_pnl: pos.unrealized_pnl,
                    liquidation_price: pos.liquidation_price,
                }
            })
            .collect())
    }

    async fn get_funding_rate(&self, symbol: &str) -> VirsResult<FundingRate> {
        warn!(
            exchange = %self.name,
            symbol = %symbol,
            "PaperExchange get_funding_rate stub — returning zero rate (paper mode, no real funding data)"
        );
        Ok(FundingRate {
            symbol: symbol.to_string(),
            rate: 0.0,
            next_funding_time: Some(Utc::now()),
        })
    }

    async fn place_order(&self, params: PlaceOrderParams) -> VirsResult<PositionOrder> {
        let order_id = Uuid::new_v4();
        let now = Utc::now();
        let is_market = params.order_type == OrderType::Market || params.price.is_none();

        if is_market {
            let fill_price = self
                .last_prices
                .get(&params.symbol)
                .map(|r| *r)
                .ok_or_else(|| {
                    tracing::error!(
                        symbol = %params.symbol,
                        "No last price available for paper market order — returning NoData instead of 0.0"
                    );
                    VirsError::Exchange(ExchangeError::no_data(format!(
                        "No last price for paper market order on {}",
                        params.symbol
                    )))
                })?;
            let pending_for_fill = PaperPendingOrder {
                id: order_id,
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                amount: params.amount,
                price: Some(fill_price),
                reduce_only: params.reduce_only,
                position_side: params.position_side,
                client_order_id: params.client_order_id.clone(),
                created_at: now,
            };
            self.update_position_on_fill(&pending_for_fill, fill_price)
                .await;


            let fee = fill_price * params.amount * 0.0005;

            let order = PositionOrder {
                id: order_id,
                position_id: params.position_id.unwrap_or(Uuid::nil()),
                exchange_order_id: Some(order_id.to_string()),
                client_order_id: params.client_order_id.clone(),
                exchange: self.name.clone(),
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                request_price: params.price,
                fill_price: if fill_price > 0.0 {
                    Some(fill_price)
                } else {
                    None
                },
                amount: params.amount,
                filled: params.amount,
                remaining: 0.0,
                status: OrderStatus::Filled,
                reduce_only: params.reduce_only,
                fee,
                fee_currency: "USDT".to_string(),
                slippage: None,
                created_at: now,
                updated_at: now,
            };

            let tx = self.price_tx.lock().await;
            if let Some(ref tx) = *tx {
                if tx
                    .send(WsFeedEvent::OrderUpdate {
                        exchange_order_id: order_id.to_string(),
                        client_order_id: params.client_order_id.clone(),
                        symbol: params.symbol.clone(),
                        status: OrderStatus::Filled,
                        filled: params.amount,
                        remaining: 0.0,
                        price: fill_price,
                        amount: params.amount,
                        commission: fee,
                        timestamp: Utc::now(),
                        position_side: params.position_side,
                    })
                    .await
                    .is_err()
                {
                    warn!(order_id = %order_id, symbol = %params.symbol, "Paper WsFeedEvent::OrderUpdate send failed — receiver dropped, event lost");
                }
            }
            Ok(order)
        } else {
            let pending = PaperPendingOrder {
                id: order_id,
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                amount: params.amount,
                price: params.price,
                reduce_only: params.reduce_only,
                position_side: params.position_side,
                client_order_id: params.client_order_id.clone(),
                created_at: now,
            };
            self.pending.insert(order_id, pending);
            let order = PositionOrder {
                id: order_id,
                position_id: params.position_id.unwrap_or(Uuid::nil()),
                exchange_order_id: Some(order_id.to_string()),
                client_order_id: params.client_order_id,
                exchange: self.name.clone(),
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                request_price: params.price,
                fill_price: None,
                amount: params.amount,
                filled: 0.0,
                remaining: params.amount,
                status: OrderStatus::Open,
                reduce_only: params.reduce_only,
                fee: 0.0,
                fee_currency: "USDT".to_string(),
                slippage: None,
                created_at: now,
                updated_at: now,
            };
            Ok(order)
        }
    }

    async fn cancel_order(&self, _symbol: &str, order_id: &str) -> VirsResult<PositionOrder> {
        let uuid = Uuid::parse_str(order_id).map_err(|_| {
            ExchangeError::Internal(format!(
                "Invalid order ID: {}",
                order_id
            ))
        })?;
        let now = Utc::now();
        match self.pending.remove(&uuid) {
            Some((_, pending)) => Ok(PositionOrder {
                id: uuid,
                position_id: Uuid::nil(),
                exchange_order_id: Some(uuid.to_string()),
                client_order_id: pending.client_order_id,
                exchange: self.name.clone(),
                symbol: pending.symbol,
                side: pending.side,
                order_type: pending.order_type,
                request_price: pending.price,
                fill_price: None,
                amount: pending.amount,
                filled: 0.0,
                remaining: pending.amount,
                status: OrderStatus::Canceled,
                reduce_only: pending.reduce_only,
                fee: 0.0,
                fee_currency: "USDT".to_string(),
                slippage: None,
                created_at: pending.created_at,
                updated_at: now,
            }),
            None => Err(VirsError::Exchange(ExchangeError::OrderNotFound(
                order_id.to_string(),
            ))),
        }
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<PositionOrder>> {
        let now = Utc::now();
        let keys: Vec<Uuid> = self
            .pending
            .iter()
            .filter(|e| symbol.is_none_or(|s| e.value().symbol == s))
            .map(|e| *e.key())
            .collect();
        let mut canceled = Vec::new();
        for key in keys {
            if let Some((_, pending)) = self.pending.remove(&key) {
                canceled.push(PositionOrder {
                    id: key,
                    position_id: Uuid::nil(),
                    exchange_order_id: Some(key.to_string()),
                    client_order_id: pending.client_order_id,
                    exchange: self.name.clone(),
                    symbol: pending.symbol,
                    side: pending.side,
                    order_type: pending.order_type,
                    request_price: pending.price,
                    fill_price: None,
                    amount: pending.amount,
                    filled: 0.0,
                    remaining: pending.amount,
                    status: OrderStatus::Canceled,
                    reduce_only: pending.reduce_only,
                    fee: 0.0,
                    fee_currency: "USDT".to_string(),
                    slippage: None,
                    created_at: pending.created_at,
                    updated_at: now,
                });
            }
        }
        Ok(canceled)
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> VirsResult<()> {

        self.configured_leverage
            .insert(symbol.to_string(), leverage);
        Ok(())
    }

    async fn get_position_mode(&self) -> VirsResult<PositionMode> {


        let registry = match &self.exchange_registry {
            Some(r) => r.clone(),
            None => return Ok(PositionMode::Hedge),
        };
        let exchange = registry
            .registered_names()
            .iter()
            .find(|n| n.contains("perpetual"))
            .and_then(|key| registry.get(key));
        match exchange {
            Some(ex) => ex.get_position_mode().await.map_err(Into::into),
            None => Ok(PositionMode::Hedge),
        }
    }

    async fn subscribe_order_updates(
        &self,
        _symbols: &[&str],
    ) -> VirsResult<OrderUpdateStream> {
        let (tx, rx) = mpsc::channel(256);
        let mut price_tx = self.price_tx.lock().await;
        *price_tx = Some(tx);
        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    async fn on_price_tick(&self, symbol: &str, price: f64) {
        PaperExchangeAdapter::on_price_tick(self, symbol, price).await;
    }


    async fn restore_positions(&self, positions: Vec<ExchangePosition>) {
        for pos in positions {

            if pos.size.abs() < 1e-8 {
                continue;
            }
            let key = format!("{}:{:?}", pos.symbol, pos.side);
            self.positions.insert(
                key.clone(),
                PaperPosition {
                    symbol: pos.symbol.clone(),
                    side: pos.side,
                    size: pos.size,
                    entry_price: pos.entry_price,
                    leverage: pos.leverage,
                    unrealized_pnl: pos.unrealized_pnl,
                    liquidation_price: pos.liquidation_price,
                },
            );

            self.last_prices.insert(pos.symbol.clone(), pos.entry_price);
        }
    }
}
