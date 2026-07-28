use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, warn};

use virs_types::enums::*;
use virs_types::exchange_pe::{ExchangePe, OrderUpdateStream};
use virs_types::market::*;
use virs_types::position::*;
use virs_types::{CcxtOrder, CcxtOrderStatus, ExecutionType, OrderResult};

use virs_error::{ExchangeError, VirsError, VirsResult};

use crate::registry::Exchanges;

#[derive(Debug, Clone)]
struct PaperPendingOrder {
    id: i64,
    symbol: String,
    side: Side,
    order_type: OrderType,
    amount: f64,
    price: Option<f64>,
    position_side: Option<PositionSide>,
    client_order_id: String,
}

type PaperPosition = ExchangePosition;

pub struct PaperExchangeAdapter {
    name: String,
    market_type: MarketType,
    pending: Arc<DashMap<i64, PaperPendingOrder>>,
    positions: Arc<DashMap<String, PaperPosition>>,
    balance: Arc<Mutex<Balance>>,
    price_tx: Arc<Mutex<Option<mpsc::Sender<WsFeedEvent>>>>,
    last_prices: Arc<DashMap<String, f64>>,
    exchange_registry: Option<Arc<Exchanges>>,
    balance_initialized: Arc<AtomicBool>,

    configured_leverage: Arc<DashMap<String, u32>>,
    order_id_counter: Arc<AtomicI64>,
    trade_id_counter: Arc<AtomicI64>,
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
            order_id_counter: Arc::new(AtomicI64::new(1)),
            trade_id_counter: Arc::new(AtomicI64::new(1)),
        }
    }

    pub fn with_exchange_registry(mut self, registry: Arc<Exchanges>) -> Self {
        self.exchange_registry = Some(registry);
        self
    }

    /// 生成递增的 order_id，与币安原生 i64 订单ID对齐
    fn next_order_id(&self) -> i64 {
        self.order_id_counter
            .fetch_add(1, Ordering::Relaxed)
    }

    /// 生成递增的 trade_id，确保同一订单多笔成交不会因 trade_id 冲突被丢弃
    fn next_trade_id(&self) -> i64 {
        self.trade_id_counter
            .fetch_add(1, Ordering::Relaxed)
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
            // ExchangePe::get_balance() 直接返回单个 USDT Balance（无需再从 Vec 中筛选）
            match ex.get_balance().await {
                Ok(usdt) => {
                    let mut balance = self.balance.lock().await;
                    if !self.balance_initialized.load(Ordering::Relaxed) {
                        balance.free = usdt.free;
                        balance.used = usdt.used;
                        balance.total = usdt.total;
                        self.balance_initialized.store(true, Ordering::Relaxed);
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
            let filled = match &order.side {
                Side::Buy => match order.price {
                    Some(price) => current_price <= price,
                    None => {
                        error!(order_id = %order.id, symbol = %order.symbol, "Buy order has no limit price — skipping");
                        continue;
                    }
                },
                Side::Sell => match order.price {
                    Some(price) => current_price >= price,
                    None => {
                        error!(order_id = %order.id, symbol = %order.symbol, "Sell order has no limit price — skipping");
                        continue;
                    }
                },
                Side::Unknown(_) => unreachable!("validate ensures side is Buy/Sell"),
            };
            if filled {
                triggered.push(order.clone());
            }
        }

        for order in &triggered {
            self.pending.remove(&order.id);
            let realized_pnl = match self.update_position_on_fill(order, current_price).await {
                Ok(pnl) => pnl,
                Err(e) => {
                    error!(
                        symbol = %order.symbol,
                        order_id = %order.id,
                        error = %e,
                        "Failed to update position on fill — skipping order"
                    );
                    continue;
                }
            };

            let fee = current_price * order.amount * 0.0002;
            let ccxt_order = CcxtOrder {
                order_id: order.id,
                client_order_id: order.client_order_id.clone(),
                symbol: order.symbol.clone(),
                side: order.side.clone(),
                order_type: order.order_type.clone(),
                position_side: order
                    .position_side
                    .clone()
                    .expect("engine always sends Some(Long/Short) to paper exchange"),
                original_order_type: Some(format!("{:?}", order.order_type)),
                status: CcxtOrderStatus::Filled,
                execution_type: ExecutionType::Trade,
                orig_qty: order.amount.to_string(),
                original_price: order.price.map(|p| p.to_string()).unwrap_or_default(),
                avg_fill_price: Some(current_price.to_string()),
                filled_qty: order.amount.to_string(),
                last_fill_qty: order.amount.to_string(),
                last_fill_price: current_price.to_string(),
                stop_price: None,
                commission: fee.to_string(),
                commission_asset: "USDT".to_string(),
                realized_pnl: Some(realized_pnl.to_string()),
                reduce_only: false,
                is_maker: true,
                close_position: None,
                time_in_force: "GTC".to_string(),
                working_type: Some("CONTRACT_PRICE".to_string()),
                bids_notional: None,
                ask_notional: None,
                activation_price: None,
                callback_rate: None,
                price_protection: Some(false),
                stp_mode: None,
                price_match_mode: None,
                gtd_auto_cancel_time: None,
                expiry_reason: None,
                si: Some(0),
                ss: Some(0),
                trade_time: chrono::Utc::now().timestamp_millis(),
                trade_id: self.next_trade_id(),
                modify_id: None,
                envelope_event_type: "ORDER_TRADE_UPDATE".to_string(),
                envelope_event_time: chrono::Utc::now().timestamp_millis(),
                envelope_transaction_time: chrono::Utc::now().timestamp_millis(),
            };
            let tx = self.price_tx.lock().await;
            if let Some(ref tx) = *tx {
                if tx
                    .send(WsFeedEvent::OrderUpdate { order: ccxt_order })
                    .await
                    .is_err()
                {
                    warn!(order_id = %order.id, symbol = %order.symbol, "Paper WsFeedEvent::OrderUpdate send failed — receiver dropped, event lost");
                }
            }
        }
    }

    async fn update_position_on_fill(
        &self,
        order: &PaperPendingOrder,
        fill_price: f64,
    ) -> VirsResult<f64> {
        let position_side = match &order.position_side {
            Some(ps) => ps.clone(),
            None => {
                return Err(VirsError::config(format!(
                    "position_side is None for order {} on {} — caller must provide position_side in Hedge mode",
                    order.id, order.symbol
                )));
            }
        };
        let key = format!("{}:{:?}", order.symbol, position_side);

        let size_delta = order.amount;

        let leverage: u32 = self
            .configured_leverage
            .get(&order.symbol)
            .map(|v| *v)
            .ok_or_else(|| {
                VirsError::config(format!(
                    "No leverage configured for {} — call set_leverage() before trading",
                    order.symbol
                ))
            })?;
        let leverage_f64 = leverage as f64;
        let notional = fill_price * order.amount;
        let margin = notional / leverage_f64;

        let is_opening = match (&order.side, &position_side) {
            (Side::Buy, PositionSide::Long) => true,
            (Side::Sell, PositionSide::Short) => true,
            (Side::Sell, PositionSide::Long) => false,
            (Side::Buy, PositionSide::Short) => false,
            (Side::Unknown(_), _) | (_, PositionSide::Unknown(_)) => {
                unreachable!("validate ensures side/position_side are known")
            }
        };

        let old_pos_info = self
            .positions
            .get(&key)
            .map(|p| (p.side.clone(), p.entry_price, p.quantity));

        let realized_pnl: f64 = match (&old_pos_info, is_opening) {
            (Some((side, entry, old_qty)), false) => {
                let closed = order.amount.min(*old_qty);
                match side {
                    PositionSide::Long => (fill_price - entry) * closed,
                    PositionSide::Short => (entry - fill_price) * closed,
                    PositionSide::Unknown(_) => {
                        unreachable!("validate ensures position_side is Long/Short")
                    }
                }
            }
            _ => 0.0,
        };

        match self.positions.get_mut(&key) {
            Some(mut pos) => {
                if is_opening {
                    let old_qty = pos.quantity;
                    let new_qty = old_qty + size_delta;
                    let total_cost = pos.entry_price * old_qty + fill_price * size_delta;
                    pos.quantity = new_qty;
                    pos.entry_price = total_cost / new_qty;
                } else {
                    let new_qty = pos.quantity - size_delta;
                    if new_qty < 1e-8 {
                        drop(pos);
                        self.positions.remove(&key);
                    } else {
                        pos.quantity = new_qty;
                    }
                }
            }
            None => {
                self.positions.insert(
                    key.clone(),
                    PaperPosition {
                        symbol: order.symbol.clone(),
                        side: position_side,
                        quantity: size_delta,
                        entry_price: fill_price,
                        margin_mode: MarginMode::Cross,
                        info: serde_json::Value::Null,
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

        Ok(realized_pnl)
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
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            format!(
                "PaperExchange does not support get_ticker for {} — no real market data in paper mode",
                symbol
            )
        )))
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
                symbol.is_none_or(|s| pos.symbol == s) && pos.quantity.abs() > 1e-8
            })
            .map(|e| {
                let pos = e.value();
                ExchangePosition {
                    symbol: pos.symbol.clone(),
                    side: pos.side.clone(),
                    quantity: pos.quantity,
                    entry_price: pos.entry_price,
                    margin_mode: pos.margin_mode,
                    info: pos.info.clone(),
                }
            })
            .collect())
    }

    async fn get_funding_rate(&self, symbol: &str) -> VirsResult<FundingRate> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            format!(
                "PaperExchange does not support get_funding_rate for {} — no real funding data in paper mode",
                symbol
            )
        )))
    }

    async fn get_klines(
        &self,
        _symbol: &str,
        _interval: &str,
        _limit: u32,
        _since: Option<i64>,
    ) -> VirsResult<Vec<Kline>> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            "PaperExchange does not support get_klines — no historical K-line data in paper mode".into(),
        )))
    }

    async fn get_klines_range(
        &self,
        _symbol: &str,
        _interval: &str,
        _start_ms: i64,
        _end_ms: i64,
    ) -> VirsResult<Vec<Kline>> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            "PaperExchange does not support get_klines_range — no historical K-line data in paper mode".into(),
        )))
    }

    async fn get_symbols(&self) -> VirsResult<Vec<String>> {
        // Paper exchange 不维护交易对元数据，返回已收到价格更新的 symbol 列表
        Ok(self
            .last_prices
            .iter()
            .map(|e| e.key().clone())
            .collect())
    }

    async fn get_min_qty(&self, _symbol: &str) -> VirsResult<f64> {
        // Paper 模式固定最小下单量，无实际交易所限制
        Ok(0.001)
    }

    async fn create_listen_key(&self) -> VirsResult<String> {
        Ok("paper-listen-key".to_string())
    }

    async fn ping(&self) -> VirsResult<bool> {
        Ok(true)
    }

    async fn get_api_restrictions(&self) -> VirsResult<ApiRestrictions> {
        Err(VirsError::Exchange(ExchangeError::NotSupported(
            "PaperExchange does not support get_api_restrictions".into(),
        )))
    }

    async fn place_order(&self, params: PlaceOrderParams) -> VirsResult<OrderResult> {
        let order_id = self.next_order_id();
        let is_market = params.order_type == OrderType::Market || params.price.is_none();

        let client_order_id = params.client_order_id.clone().ok_or_else(|| {
            VirsError::config(format!(
                "client_order_id is required for place_order on {} — \
                 all callers must generate one via client_order_id::format_* before placing orders",
                params.symbol
            ))
        })?;

        if is_market {
            let fill_price = self
                .last_prices
                .get(&params.symbol)
                .map(|r| *r)
                .ok_or_else(|| {
                    VirsError::Exchange(ExchangeError::no_data(format!(
                        "No last price for paper market order on {}",
                        params.symbol
                    )))
                })?;
            let pending_for_fill = PaperPendingOrder {
                id: order_id,
                symbol: params.symbol.clone(),
                side: params.side.clone(),
                order_type: params.order_type.clone(),
                amount: params.amount,
                price: Some(fill_price),
                position_side: params.position_side.clone(),
                client_order_id: client_order_id.clone(),
            };
            let realized_pnl = self
                .update_position_on_fill(&pending_for_fill, fill_price)
                .await?;

            let fee = fill_price * params.amount * 0.0005;

            let order_result = OrderResult {
                order_id: order_id.to_string(),
                client_order_id: client_order_id.clone(),
            };

            let ccxt_order = CcxtOrder {
                order_id,
                client_order_id,
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type.clone(),
                position_side: params
                    .position_side
                    .expect("engine always sends Some(Long/Short) to paper exchange"),
                original_order_type: Some(format!("{:?}", params.order_type)),
                status: CcxtOrderStatus::Filled,
                execution_type: ExecutionType::Trade,
                orig_qty: params.amount.to_string(),
                original_price: params.price.map(|p| p.to_string()).unwrap_or_default(),
                avg_fill_price: Some(fill_price.to_string()),
                filled_qty: params.amount.to_string(),
                last_fill_qty: params.amount.to_string(),
                last_fill_price: fill_price.to_string(),
                stop_price: None,
                commission: fee.to_string(),
                commission_asset: "USDT".to_string(),
                realized_pnl: Some(realized_pnl.to_string()),
                reduce_only: false,
                is_maker: false,
                close_position: None,
                time_in_force: "GTC".to_string(),
                working_type: Some("CONTRACT_PRICE".to_string()),
                bids_notional: None,
                ask_notional: None,
                activation_price: None,
                callback_rate: None,
                price_protection: Some(false),
                stp_mode: None,
                price_match_mode: None,
                gtd_auto_cancel_time: None,
                expiry_reason: None,
                si: Some(0),
                ss: Some(0),
                trade_time: chrono::Utc::now().timestamp_millis(),
                trade_id: self.next_trade_id(),
                modify_id: None,
                envelope_event_type: "ORDER_TRADE_UPDATE".to_string(),
                envelope_event_time: chrono::Utc::now().timestamp_millis(),
                envelope_transaction_time: chrono::Utc::now().timestamp_millis(),
            };

            let tx = self.price_tx.lock().await;
            if let Some(ref tx) = *tx {
                if tx
                    .send(WsFeedEvent::OrderUpdate { order: ccxt_order })
                    .await
                    .is_err()
                {
                    warn!(order_id = %order_id, symbol = %params.symbol, "Paper WsFeedEvent::OrderUpdate send failed — receiver dropped, event lost");
                }
            }
            Ok(order_result)
        } else {
            let pending = PaperPendingOrder {
                id: order_id,
                symbol: params.symbol.clone(),
                side: params.side,
                order_type: params.order_type,
                amount: params.amount,
                price: params.price,
                position_side: params.position_side,
                client_order_id: client_order_id.clone(),
            };
            self.pending.insert(order_id, pending);
            // 限价单挂单未成交，不发送 WS 推送
            let order_result = OrderResult {
                order_id: order_id.to_string(),
                client_order_id,
            };
            Ok(order_result)
        }
    }

    async fn cancel_order(&self, _symbol: &str, order_id: &str) -> VirsResult<OrderResult> {
        let id = order_id
            .parse::<i64>()
            .map_err(|_| ExchangeError::Internal(format!("Invalid order ID: {}", order_id)))?;
        match self.pending.remove(&id) {
            Some((_, pending)) => Ok(OrderResult {
                order_id: order_id.to_string(),
                client_order_id: pending.client_order_id,
            }),
            None => Err(VirsError::Exchange(ExchangeError::OrderNotFound(
                order_id.to_string(),
            ))),
        }
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<OrderResult>> {
        let keys: Vec<i64> = self
            .pending
            .iter()
            .filter(|e| symbol.is_none_or(|s| e.value().symbol == s))
            .map(|e| *e.key())
            .collect();
        let mut canceled = Vec::new();
        for key in keys {
            if let Some((_, pending)) = self.pending.remove(&key) {
                canceled.push(OrderResult {
                    order_id: key.to_string(),
                    client_order_id: pending.client_order_id,
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

    async fn subscribe_order_updates(&self, _symbols: &[&str]) -> VirsResult<OrderUpdateStream> {
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
            if pos.quantity.abs() < 1e-8 {
                continue;
            }
            let key = format!("{}:{:?}", pos.symbol, pos.side);
            self.positions.insert(
                key.clone(),
                PaperPosition {
                    symbol: pos.symbol.clone(),
                    side: pos.side,
                    quantity: pos.quantity,
                    entry_price: pos.entry_price,
                    margin_mode: pos.margin_mode,
                    info: pos.info.clone(),
                },
            );

            self.last_prices.insert(pos.symbol.clone(), pos.entry_price);
        }
    }
}
