use std::sync::{Arc, Mutex, RwLock};

use chrono::Utc;
use dashmap::DashMap;
use futures_util::StreamExt;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};
use uuid::Uuid;

use virs_types::enums::*;
use virs_types::exchange_pe::{ExchangePe, OrderUpdateStream};
use virs_types::market::ExchangePosition;
use virs_types::position::*;
use virs_error::{VirsError, VirsResult};

use crate::persistence::PositionPersistence;
use crate::tracker::PnlTracker;


fn recover_lock<T>(lock: std::sync::LockResult<T>) -> T {
    lock.unwrap_or_else(|_| {
        error!(
            "Lock poisoned — a thread panicked while holding a lock. \
             Refusing to return potentially inconsistent data. \
             Panicking to prevent data corruption. \
             Restart the engine immediately."
        );
        panic!("Lock poisoned — cannot continue safely. Restart the engine.");
    })
}

macro_rules! persist {
    ($expr:expr, $label:expr, $max_retries:expr, $base_ms:expr) => {
        let mut attempts = 0u32;
        loop {
            match $expr.await {
                Ok(()) => break,
                Err(e) => {
                    attempts += 1;
                    if attempts >= $max_retries {
                        error!(error = %e, attempts, $label);
                        break;
                    }
                    warn!(error = %e, attempt = attempts, $label);
                    tokio::time::sleep(std::time::Duration::from_millis($base_ms * attempts as u64)).await;
                }
            }
        }
    };
}


pub(crate) struct EngineInner {
    pub(crate) exchange: Arc<dyn ExchangePe>,
    pub(crate) persistence: Box<dyn PositionPersistence>,
    pub(crate) positions: DashMap<(String, String, PositionSide), Position>,
    pub(crate) orders: DashMap<Uuid, PositionOrder>,
    pub(crate) event_tx: broadcast::Sender<EngineEvent>,
    pub(crate) tracker: Mutex<PnlTracker>,
    pub(crate) state: RwLock<EngineState>,
    pub(crate) exchange_order_id_index: DashMap<String, Uuid>,


    pub(crate) client_order_id_index: DashMap<String, Uuid>,
    pub(crate) position_id_index: DashMap<Uuid, (String, String, PositionSide)>,


    pub(crate) close_order_timeout: Duration,

    pub(crate) persist_max_retries: u32,

    pub(crate) persist_retry_base_ms: u64,
}

impl EngineInner {
    fn emit_event(&self, event: EngineEvent) {
        if self.event_tx.receiver_count() > 0 {
            if self.event_tx.send(event).is_err() {
                tracing::debug!("EngineEvent broadcast — receiver dropped between check and send");
            }
        }
    }

    fn is_running(&self) -> bool {
        self.get_state().is_running()
    }

    fn set_state(&self, new_state: EngineState) {
        *recover_lock(self.state.write()) = new_state;
    }

    fn get_state(&self) -> EngineState {
        *recover_lock(self.state.read())
    }
}


pub struct PositionEngine {
    inner: Arc<EngineInner>,
    cmd_tx: mpsc::Sender<EngineCommand>,
    cmd_rx: Option<mpsc::Receiver<EngineCommand>>,
}

impl Clone for PositionEngine {


    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            cmd_tx: self.cmd_tx.clone(),
            cmd_rx: None,
        }
    }
}

impl PositionEngine {


    pub fn new(
        exchange: Box<dyn ExchangePe>,
        persistence: Box<dyn PositionPersistence>,
        close_order_timeout: Duration,
        persist_max_retries: u32,
        persist_retry_base_ms: u64,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let event_tx = broadcast::channel(256).0;
        let exchange: Arc<dyn ExchangePe> = Arc::from(exchange);

        let inner = EngineInner {
            persistence,
            tracker: Mutex::new(PnlTracker::new(0.0)),
            state: RwLock::new(EngineState::Created),
            exchange,
            event_tx,
            positions: DashMap::new(),
            orders: DashMap::new(),
            exchange_order_id_index: DashMap::new(),
            client_order_id_index: DashMap::new(),
            position_id_index: DashMap::new(),
            close_order_timeout,
            persist_max_retries,
            persist_retry_base_ms,
        };

        Self {
            inner: Arc::new(inner),
            cmd_tx,
            cmd_rx: Some(cmd_rx),
        }
    }


    pub fn command_sender(&self) -> mpsc::Sender<EngineCommand> {
        self.cmd_tx.clone()
    }


    pub fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent> {
        self.inner.event_tx.subscribe()
    }


    pub fn event_sender(&self) -> broadcast::Sender<EngineEvent> {
        self.inner.event_tx.clone()
    }


    pub fn get_all_positions(&self) -> Vec<Position> {
        self.inner
            .positions
            .iter()
            .map(|r| r.value().clone())
            .collect()
    }


    pub fn get_open_position_by_symbol(&self, symbol: &str) -> Option<Position> {
        self.inner
            .positions
            .iter()
            .filter(|r| r.value().symbol == symbol && r.value().is_open())
            .map(|r| r.value().clone())
            .next()
    }


    pub fn exchange(&self) -> Arc<dyn ExchangePe> {
        Arc::clone(&self.inner.exchange)
    }


    pub async fn run(&mut self) -> VirsResult<()> {


        self.recover_state().await?;


        let symbols: Vec<String> = self
            .inner
            .positions
            .iter()
            .map(|r| r.value().symbol.clone())
            .collect();
        let unique_symbols: Vec<String> = symbols.into_iter().collect();
        let unique_symbol_refs: Vec<&str> = unique_symbols.iter().map(|s| s.as_str()).collect();
        let ws_feed_rx = self
            .inner
            .exchange
            .subscribe_order_updates(&unique_symbol_refs)
            .await?;


        self.inner.set_state(EngineState::Running);
        info!("Position engine started");


        let cmd_rx = self
            .cmd_rx
            .take()
            .ok_or(VirsError::Http { status: 500, message: "Channel closed".to_string() })?;
        let inner = Arc::clone(&self.inner);

        let mut cmd_handle = tokio::spawn(command_loop(inner.clone(), cmd_rx));
        let mut ws_handle = tokio::spawn(ws_feed_loop(inner.clone(), ws_feed_rx));


        let _ = tokio::select! {
            r = &mut cmd_handle => r,
            r = &mut ws_handle => r,
        };


        self.inner.set_state(EngineState::ShuttingDown);


        let timeout = Duration::from_secs(5);
        let _ = tokio::time::timeout(timeout, async {
            let _ = tokio::join!(cmd_handle, ws_handle);
        }).await;

        self.inner.set_state(EngineState::Stopped);
        info!("Position engine stopped");
        Ok(())
    }


    pub fn stop(&self) {
        self.inner.set_state(EngineState::ShuttingDown);
        info!("Position engine stop requested");
    }


    async fn recover_state(&self) -> VirsResult<()> {
        let open_positions = self.inner.persistence.get_open_positions().await?;
        for pos in &open_positions {
            let key = (pos.exchange.clone(), pos.symbol.clone(), pos.side);
            self.inner.position_id_index.insert(pos.id, key.clone());
            self.inner.positions.insert(key, pos.clone());
        }


        let exchange_positions: Vec<ExchangePosition> = open_positions
            .iter()
            .map(|p| ExchangePosition {
                symbol: p.symbol.clone(),
                side: p.side,
                size: p.size,
                entry_price: p.entry_price,
                leverage: p.leverage,
                unrealized_pnl: p.unrealized_pnl,
                liquidation_price: p.liquidation_price,
            })
            .collect();
        self.inner
            .exchange
            .restore_positions(exchange_positions)
            .await;

        self.full_sync().await;
        Ok(())
    }

    async fn full_sync(&self) {
        let exchange_name = self.inner.exchange.name().to_string();

        match self.inner.exchange.get_positions(None).await {
            Ok(exchange_positions) => {
                for ep in &exchange_positions {
                    let key = (exchange_name.clone(), ep.symbol.clone(), ep.side);
                    match self.inner.positions.get(&key) {
                        Some(local) => {
                            let mut pos = local.value().clone();
                            pos.current_price = ep.entry_price;
                            pos.unrealized_pnl = ep.unrealized_pnl;
                            pos.liquidation_price = ep.liquidation_price;
                            pos.updated_at = Utc::now();
                            drop(local);
                            persist!(
                                self.inner.persistence.upsert_position(&pos),
                                "Failed to persist position in full_sync",
                                self.inner.persist_max_retries,
                                self.inner.persist_retry_base_ms
                            );
                            self.inner.positions.insert(key, pos.clone());

                            self.inner
                                .emit_event(EngineEvent::PositionUpdated { position: pos });
                        }
                        None => {
                            let now = Utc::now();
                            let position = Position {
                                id: Uuid::new_v4(),
                                strategy_id: None,
                                exchange: exchange_name.clone(),
                                symbol: ep.symbol.clone(),
                                side: ep.side,
                                status: PositionStatus::Open,
                                size: ep.size,
                                entry_price: ep.entry_price,
                                current_price: ep.entry_price,
                                leverage: ep.leverage,
                                margin: if ep.leverage > 0 {
                                    ep.size * ep.entry_price / ep.leverage as f64
                                } else {
                                    0.0
                                },
                                unrealized_pnl: ep.unrealized_pnl,
                                realized_pnl: 0.0,
                                stop_loss: None,
                                take_profit: None,
                                liquidation_price: ep.liquidation_price,
                                opened_at: now,
                                updated_at: now,
                                closed_at: None,
                                metadata: serde_json::Value::Null,
                            };
                            let new_key = (
                                position.exchange.clone(),
                                position.symbol.clone(),
                                position.side,
                            );
                            self.inner
                                .position_id_index
                                .insert(position.id, new_key.clone());
                            persist!(
                                self.inner.persistence.upsert_position(&position),
                                "Failed to persist new position in full_sync",
                                self.inner.persist_max_retries,
                                self.inner.persist_retry_base_ms
                            );
                            self.inner.positions.insert(new_key, position.clone());

                            self.inner.emit_event(EngineEvent::PositionOpened {
                                position: position.clone(),
                            });
                            self.inner
                                .emit_event(EngineEvent::PositionUpdated { position });
                        }
                    }
                }
                self.inner.emit_event(EngineEvent::PositionSynced {
                    positions: exchange_positions,
                });
            }
            Err(e) => {
                warn!(error = %e, "Failed to sync positions during recovery");
            }
        }
    }
}


pub(crate) async fn command_loop(
    inner: Arc<EngineInner>,
    mut cmd_rx: mpsc::Receiver<EngineCommand>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            EngineCommand::OpenPosition {
                exchange,
                symbol,
                side,
                order_side,
                size,
                leverage,
                order_type,
                price,
                stop_loss,
                take_profit,
                strategy_id,
            } => {
                handle_open_position(
                    &inner,
                    exchange,
                    symbol,
                    side,
                    order_side,
                    size,
                    leverage,
                    order_type,
                    price,
                    stop_loss,
                    take_profit,
                    strategy_id,
                )
                .await;
            }
            EngineCommand::ClosePosition {
                position_id,
                order_type,
                price,
                strategy_id,
            } => {
                handle_close_position(&inner, position_id, order_type, price, strategy_id).await;
            }
            EngineCommand::PlaceOrder { params } => {
                handle_place_order(&inner, params).await;
            }
            EngineCommand::CancelOrder { order_id } => {
                handle_cancel_order(&inner, order_id).await;
            }
            EngineCommand::CancelAllOrders {
                position_id,
                symbol,
            } => {
                handle_cancel_all_orders(&inner, position_id, symbol).await;
            }
            EngineCommand::CloseAllPositions { symbol } => {
                handle_close_all_positions(&inner, &symbol).await;
            }
            EngineCommand::PriceTick { symbol, price } => {
                inner.exchange.on_price_tick(&symbol, price).await;
            }
        }
    }
}


pub(crate) async fn ws_feed_loop(inner: Arc<EngineInner>, mut ws_rx: OrderUpdateStream) {
    loop {
        tokio::select! {
            event = ws_rx.next() => {
                match event {
                    Some(WsFeedEvent::OrderUpdate {
                        exchange_order_id,
                        client_order_id,
                        symbol,
                        status,
                        filled,
                        remaining,
                        price,
                        amount,
                        commission,
                        timestamp,
                        position_side,
                    }) => {
                        handle_ws_order_update(
                            &inner,
                            &exchange_order_id,
                            client_order_id.as_deref(),
                            &symbol,
                            status,
                            filled,
                            remaining,
                            price,
                            amount,
                            commission,
                            timestamp,
                            position_side,
                        )
                        .await;
                    }
                    Some(WsFeedEvent::ConnectionChanged { .. }) => {}
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                if !inner.is_running() {
                    break;
                }
            }
        }
    }
}


pub(crate) async fn handle_ws_order_update(
    inner: &Arc<EngineInner>,
    exchange_order_id: &str,
    client_order_id: Option<&str>,
    symbol: &str,
    status: OrderStatus,
    filled: f64,
    remaining: f64,
    price: f64,
    amount: f64,
    commission: f64,
    timestamp: chrono::DateTime<Utc>,
    ws_position_side: Option<PositionSide>,
) {

    let (order_id, position_id, prev_filled, is_reduce_only) = {

        let order_id_opt = inner
            .exchange_order_id_index
            .get(exchange_order_id)
            .map(|r| *r.value());


        let order_id = match order_id_opt {
            Some(id) => id,
            None => {
                if let Some(cid) = client_order_id {
                    if let Some(id) = inner
                        .client_order_id_index
                        .get(cid)
                        .map(|r| *r.value())
                    {

                        inner
                            .exchange_order_id_index
                            .insert(exchange_order_id.to_string(), id);
                        tracing::debug!(
                            exchange_order_id,
                            client_order_id = cid,
                            order_uuid = %id,
                            "WS order matched by client_order_id — backfilling exchange_order_id_index"
                        );
                        id
                    } else {
                        warn!(
                            exchange_order_id,
                            client_order_id = cid,
                            "Received order update for unknown order — dual-index miss (both exchange_order_id and client_order_id not found)"
                        );
                        return;
                    }
                } else {
                    warn!(
                        exchange_order_id,
                        "Received order update for unknown order — no client_order_id available for fallback"
                    );
                    return;
                }
            }
        };
        let order = match inner.orders.get(&order_id) {
            Some(o) => o,
            None => {
                warn!(exchange_order_id, "Order index points to missing order — state inconsistency detected");
                return;
            }
        };

        let is_reduce_only = order.reduce_only || {
            if let Some(ref ws_ps) = ws_position_side {
                matches!((&order.side, ws_ps), (Side::Sell, PositionSide::Long) | (Side::Buy, PositionSide::Short))
            } else {
                false
            }
        };

        (order.id, order.position_id, order.filled, is_reduce_only)
    };


    let pos_key_opt = inner
        .position_id_index
        .get(&position_id)
        .map(|r| r.value().clone());


    if let Some(ref ws_ps) = ws_position_side {
        if let Some(ref pos_key) = pos_key_opt {
            let pos_side = &pos_key.2;
            if pos_side != ws_ps {
                warn!(exchange_order_id, order_id = %order_id, ws_position_side = ?ws_ps, local_position_side = ?pos_side, "WS position_side mismatch");
            }
        }
    }


    {
        if let Some(mut order) = inner.orders.get_mut(&order_id) {
            order.filled = filled;
            order.remaining = remaining;
            order.fill_price = Some(price);
            order.fee = commission;
            order.status = status;
            order.updated_at = timestamp;
            drop(order);
        }
    }


    let current_order_opt = if matches!(
        status,
        OrderStatus::PartiallyFilled | OrderStatus::Filled
    ) {
        inner.orders.get(&order_id).map(|r| r.value().clone())
    } else {
        None
    };


    if matches!(status, OrderStatus::PartiallyFilled | OrderStatus::Filled) {
        let trade_fill = filled - prev_filled;

        if trade_fill < 0.0 {
            warn!(order_id = %order_id, prev_filled, new_filled = filled, "WS order update out of order: filled decreased");
            return;
        }

        if trade_fill > 0.0 {
            let (pnl, trade_side) = {
                match &pos_key_opt {
                    Some(key) => {
                        let pos_entry = inner.positions.get(key);
                        match pos_entry {
                            Some(pe) => {
                                let pos = pe.value();
                                if is_reduce_only {
                                    let p = match pos.side {
                                        PositionSide::Long => {
                                            (price - pos.entry_price) * trade_fill
                                        }
                                        PositionSide::Short => {
                                            (pos.entry_price - price) * trade_fill
                                        }
                                    };
                                    let side = match pos.side {
                                        PositionSide::Long => Side::Sell,
                                        PositionSide::Short => Side::Buy,
                                    };
                                    (p, side)
                                } else {
                                    let side = match pos.side {
                                        PositionSide::Long => Side::Buy,
                                        PositionSide::Short => Side::Sell,
                                    };
                                    (0.0, side)
                                }
                            }
                            None => (0.0, Side::Buy),
                        }
                    }
                    None => (0.0, Side::Buy),
                }
            };


            if price <= 0.0 {
                error!(
                    order_id = %order_id,
                    symbol = %symbol,
                    price = price,
                    "WS order update has invalid price (<=0.0) — skipping Trade record to prevent 0.0 price propagation"
                );
                return;
            }

            let trade = Trade {
                id: Uuid::new_v4(),
                position_id,
                order_id,
                exchange: inner.exchange.name().to_string(),
                symbol: symbol.to_string(),
                side: trade_side,
                price,
                amount: trade_fill,
                fee: commission,
                fee_currency: String::new(),
                pnl,
                trade_type: if is_reduce_only {
                    TradeType::Close
                } else {
                    TradeType::Open
                },
                created_at: timestamp,
            };

            {
                recover_lock(inner.tracker.lock()).record_trade(&trade);
            }

            if pnl != 0.0 {
                if let Some(key) = &pos_key_opt {
                    if let Some(mut pos) = inner.positions.get_mut(key) {
                        pos.realized_pnl += pnl;
                    }
                }
            }

            let current_order = current_order_opt
                .clone()
                .unwrap_or_else(|| PositionOrder {
                    id: order_id,
                    position_id,
                    exchange_order_id: Some(exchange_order_id.to_string()),
                    client_order_id: None,
                    exchange: inner.exchange.name().to_string(),
                    symbol: symbol.to_string(),
                    side: Side::Buy,
                    order_type: OrderType::Market,
                    request_price: None,
                    fill_price: Some(price),
                    amount,
                    filled,
                    remaining,
                    status,
                    reduce_only: false,
                    fee: commission,
                    fee_currency: String::new(),
                    slippage: None,
                    created_at: timestamp,
                    updated_at: timestamp,
                });

            match status {
                OrderStatus::Filled => {
                    inner.emit_event(EngineEvent::OrderFilled {
                        order: current_order,
                        trade: trade.clone(),
                    });
                }
                OrderStatus::PartiallyFilled => {
                    inner.emit_event(EngineEvent::OrderPartiallyFilled {
                        order: current_order,
                        trade,
                    });
                }
                _ => {}
            }
        }
    }


    if status.is_filled() {
        let pos_entry = match &pos_key_opt {
            Some(key) => inner.positions.get(key).map(|r| r.value().clone()),
            None => None,
        };

        if let Some(mut position) = pos_entry {
            let order = current_order_opt.clone();

            if let Some(order) = order {
                if is_reduce_only {
                    position.size -= order.filled;
                    if position.size.abs() < 1e-8 {
                        position.size = 0.0;
                        position.status = PositionStatus::Closed;
                        position.closed_at = Some(timestamp);
                        inner.emit_event(EngineEvent::PositionClosed {
                            position: position.clone(),
                        });
                        inner.emit_event(EngineEvent::PositionUpdated {
                            position: position.clone(),
                        });
                    } else {
                        position.status = PositionStatus::Open;
                        inner.emit_event(EngineEvent::PositionUpdated {
                            position: position.clone(),
                        });
                    }
                } else {
                    let old_size = position.size;
                    position.size += order.filled;
                    if let Some(fp) = order.fill_price {
                        if old_size > 0.0 && position.entry_price > 0.0 {
                            let total_cost = position.entry_price * old_size + fp * order.filled;
                            position.entry_price = total_cost / position.size;
                        } else {
                            position.entry_price = fp;
                        }
                        position.current_price = fp;
                    }
                    position.status = PositionStatus::Open;
                }

                position.updated_at = timestamp;
                let key = (
                    position.exchange.clone(),
                    position.symbol.clone(),
                    position.side,
                );

                if position.status == PositionStatus::Closed {
                    inner.position_id_index.remove(&position.id);
                    inner.positions.remove(&key);
                } else {
                    inner.positions.insert(key, position.clone());
                }
                persist!(
                    inner.persistence.upsert_position(&position),
                    "Failed to persist position after order fill",
                    inner.persist_max_retries,
                    inner.persist_retry_base_ms
                );
            }
        }
    }
}


pub(crate) async fn handle_open_position(
    inner: &Arc<EngineInner>,
    exchange: String,
    symbol: String,
    side: PositionSide,
    _order_side: Side,
    size: f64,
    leverage: u32,
    order_type: OrderType,
    price: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    strategy_id: Option<String>,
) {
    let exchange_name = if exchange.is_empty() {
        inner.exchange.name().to_string()
    } else {
        exchange
    };
    let key = (exchange_name.clone(), symbol.clone(), side);


    if let Some(existing) = inner.positions.get(&key) {
        let position_id = existing.id;
        drop(existing);

        let resolved_side = match side {
            PositionSide::Long => Side::Buy,
            PositionSide::Short => Side::Sell,
        };

        let mut params = PlaceOrderParams {
            symbol: symbol.clone(),
            side: resolved_side,
            order_type,
            amount: size,
            price,
            reduce_only: false,
            position_side: Some(side),
            position_id: Some(position_id),
            client_order_id: strategy_id.clone(),
        };
        resolve_position_side_for_hedge(&mut params);
        let reduce_only = params.reduce_only;

        match inner.exchange.place_order(params).await {
            Ok(mut order) => {
                order.reduce_only = reduce_only;
                if let Some(ref eoid) = order.exchange_order_id {
                    inner.exchange_order_id_index.insert(eoid.clone(), order.id);
                }
                if let Some(ref cid) = order.client_order_id {
                    inner.client_order_id_index.insert(cid.clone(), order.id);
                }
                inner.orders.insert(order.id, order.clone());


                if order.filled > 0.0 {
                    match order.fill_price.filter(|p| *p > 0.0) {
                        Some(fill_price) => {
                            let trade = Trade {
                                id: Uuid::new_v4(),
                                position_id,
                                order_id: order.id,
                                exchange: exchange_name.clone(),
                                symbol: symbol.clone(),
                                side: resolved_side,
                                price: fill_price,
                                amount: order.filled,
                                fee: order.fee,
                                fee_currency: order.fee_currency.clone(),
                                pnl: 0.0,
                                trade_type: TradeType::Open,
                                created_at: Utc::now(),
                            };
                            inner.emit_event(EngineEvent::OrderFilled {
                                order: order.clone(),
                                trade,
                            });
                        }
                        None => {
                            error!(order_id = %order.id, "Order filled but no valid fill_price — skipping Trade record to prevent 0.0 price propagation");
                        }
                    }
                } else {
                    inner.emit_event(EngineEvent::OrderPlaced {
                        order: order.clone(),
                    });
                }
            }
            Err(e) => {
                let msg = format!("Failed to place order: {}", e);
                error!(error = %e, symbol = %symbol, "Failed to place order for existing position");
                inner.emit_event(EngineEvent::OrderFailed {
                    order_id: position_id,
                    reason: msg,
                });
            }
        }
        return;
    }


    if leverage == 0 {
        let msg = "leverage must be > 0".to_string();
        error!(symbol = %symbol, "open_position rejected: leverage is 0");
        inner.emit_event(EngineEvent::OrderFailed {
            order_id: Uuid::nil(),
            reason: msg,
        });
        return;
    }

    if let Err(e) = inner.exchange.set_leverage(&symbol, leverage).await {
        let msg = format!("Failed to set leverage: {}", e);
        error!(error = %e, symbol = %symbol, leverage = leverage, "Failed to set leverage");
        inner.emit_event(EngineEvent::RiskAlert {
            level: "warning".to_string(),
            message: format!(
                "Leverage inconsistency: {}x requested for {} but exchange returned error: {}",
                leverage, symbol, e
            ),
        });
        inner.emit_event(EngineEvent::OrderFailed {
            order_id: Uuid::nil(),
            reason: msg,
        });
        return;
    }

    let lev = leverage;

    let now = Utc::now();
    let position_id = Uuid::new_v4();
    let mut position = Position {
        id: position_id,
        strategy_id: strategy_id.clone(),
        exchange: exchange_name.clone(),
        symbol: symbol.clone(),
        side,
        status: PositionStatus::Opening,
        size: 0.0,
        entry_price: 0.0,
        current_price: 0.0,
        leverage: lev,
        margin: 0.0,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss,
        take_profit,
        liquidation_price: None,
        opened_at: now,
        updated_at: now,
        closed_at: None,
        metadata: serde_json::Value::Null,
    };

    let resolved_side = match side {
        PositionSide::Long => Side::Buy,
        PositionSide::Short => Side::Sell,
    };

    let mut params = PlaceOrderParams {
        symbol: symbol.clone(),
        side: resolved_side,
        order_type,
        amount: size,
        price,
        reduce_only: false,
        position_side: Some(side),
        position_id: Some(position_id),
        client_order_id: strategy_id.clone(),
    };
    resolve_position_side_for_hedge(&mut params);
    let reduce_only = params.reduce_only;

    match inner.exchange.place_order(params).await {
        Ok(mut order) => {
            order.reduce_only = reduce_only;

            order.position_id = position_id;
            position.status = PositionStatus::Open;
            position.size = order.filled;


            let fill_price = if order.filled > 0.0 {
                match order.fill_price {
                    Some(p) if p > 0.0 => p,
                    _ => {
                        error!(
                            order_id = %order.id,
                            filled = order.filled,
                            "Order is filled but has no valid fill_price — \
                             refusing to update position to prevent data corruption."
                        );


                        position.status = PositionStatus::Opening;
                        return;
                    }
                }
            } else {


                match order.fill_price {
                    Some(p) if p > 0.0 => p,
                    _ => {
                        tracing::warn!(
                            order_id = %order.id,
                            filled = order.filled,
                            "Order is not filled and has no fill_price — skipping position update to prevent zero-cost position"
                        );
                        position.status = PositionStatus::Opening;
                        return;
                    }
                }
            };
            position.entry_price = fill_price;
            position.current_price = position.entry_price;
            position.margin = if lev > 0 {
                position.size * position.entry_price / lev as f64
            } else {
                0.0
            };

            inner.position_id_index.insert(position.id, key.clone());
            inner.positions.insert(key, position.clone());
            if let Some(ref eoid) = order.exchange_order_id {
                inner.exchange_order_id_index.insert(eoid.clone(), order.id);
            }
            if let Some(ref cid) = order.client_order_id {
                inner.client_order_id_index.insert(cid.clone(), order.id);
            }
            inner.orders.insert(order.id, order.clone());

            persist!(
                inner.persistence.upsert_position(&position),
                "Failed to persist position in open_position",
                inner.persist_max_retries,
                inner.persist_retry_base_ms
            );

            inner.emit_event(EngineEvent::PositionOpened {
                position: position.clone(),
            });
            inner.emit_event(EngineEvent::PositionUpdated {
                position: position.clone(),
            });


            if order.filled > 0.0 {
                match order.fill_price.filter(|p| *p > 0.0) {
                    Some(fill_price) => {
                        let trade = Trade {
                            id: Uuid::new_v4(),
                            position_id,
                            order_id: order.id,
                            exchange: exchange_name.clone(),
                            symbol: symbol.clone(),
                            side: resolved_side,
                            price: fill_price,
                            amount: order.filled,
                            fee: order.fee,
                            fee_currency: order.fee_currency.clone(),
                            pnl: 0.0,
                            trade_type: TradeType::Open,
                            created_at: Utc::now(),
                        };
                        inner.emit_event(EngineEvent::OrderFilled {
                            order: order.clone(),
                            trade,
                        });
                        info!(position_id = %position.id, symbol = %symbol, side = ?side, size = order.filled, "Position opened and filled");
                    }
                    None => {
                        error!(order_id = %order.id, "Order filled but no valid fill_price — skipping Trade record to prevent 0.0 price propagation");
                    }
                }
            } else {
                inner.emit_event(EngineEvent::OrderPlaced {
                    order: order.clone(),
                });
                info!(position_id = %position.id, symbol = %symbol, side = ?side, "Position opened, order pending");
            }
        }
        Err(e) => {
            let msg = format!("Failed to place order: {}", e);
            error!(error = %e, symbol = %symbol, "Failed to place opening order");
            inner.emit_event(EngineEvent::OrderFailed {
                order_id: position_id,
                reason: msg,
            });
        }
    }
}

pub(crate) async fn handle_close_position(
    inner: &Arc<EngineInner>,
    position_id: Uuid,
    order_type: OrderType,
    price: Option<f64>,
    strategy_id: Option<String>,
) {
    let position = {
        let pos_key = inner
            .position_id_index
            .get(&position_id)
            .map(|r| r.value().clone());
        match pos_key {
            Some(key) => inner.positions.get(&key).map(|r| r.value().clone()),
            None => None,
        }
    };

    let position = match position {
        Some(p) => p,
        None => {
            let msg = format!("Position not found: {}", position_id);
            warn!(msg);
            inner.emit_event(EngineEvent::OrderFailed {
                order_id: Uuid::nil(),
                reason: msg,
            });
            return;
        }
    };

    if position.size == 0.0 {
        inner.emit_event(EngineEvent::OrderFailed {
            order_id: Uuid::nil(),
            reason: format!("Position {} has zero size", position_id),
        });
        return;
    }

    let close_side = match position.side {
        PositionSide::Long => Side::Sell,
        PositionSide::Short => Side::Buy,
    };

    let mut params = PlaceOrderParams {
        symbol: position.symbol.clone(),
        side: close_side,
        order_type,
        amount: position.size,
        price,
        reduce_only: true,
        position_side: Some(position.side),
        position_id: Some(position.id),
        client_order_id: strategy_id.clone(),
    };
    resolve_position_side_for_hedge(&mut params);
    let reduce_only = params.reduce_only;


    let close_order_timeout = inner.close_order_timeout;
    match tokio::time::timeout(close_order_timeout, inner.exchange.place_order(params)).await {
        Ok(Ok(mut order)) => {
            order.reduce_only = reduce_only;

            order.position_id = position_id;
            if let Some(ref eoid) = order.exchange_order_id {
                inner.exchange_order_id_index.insert(eoid.clone(), order.id);
            }
            if let Some(ref cid) = order.client_order_id {
                inner.client_order_id_index.insert(cid.clone(), order.id);
            }
            inner.orders.insert(order.id, order.clone());


            if order.filled > 0.0 {
                match order.fill_price.filter(|p| *p > 0.0) {
                    Some(fill_price) => {
                        let trade = Trade {
                            id: Uuid::new_v4(),
                            position_id,
                            order_id: order.id,
                            exchange: position.exchange.clone(),
                            symbol: position.symbol.clone(),
                            side: close_side,
                            price: fill_price,
                            amount: order.filled,
                            fee: order.fee,
                            fee_currency: order.fee_currency.clone(),
                            pnl: 0.0,
                            trade_type: TradeType::Close,
                            created_at: Utc::now(),
                        };
                        inner.emit_event(EngineEvent::OrderFilled {
                            order: order.clone(),
                            trade,
                        });
                        info!(position_id = %position_id, symbol = %position.symbol, "Close order filled");


                        let key = (
                            position.exchange.clone(),
                            position.symbol.clone(),
                            position.side,
                        );
                        if let Some(mut pos) = inner.positions.get_mut(&key) {
                            pos.size = 0.0;
                            pos.status = PositionStatus::Closed;
                            pos.closed_at = Some(Utc::now());
                            pos.updated_at = Utc::now();
                            let closed_pos = pos.clone();
                            drop(pos);
                            inner.position_id_index.remove(&closed_pos.id);
                            inner.positions.remove(&key);
                            persist!(
                                inner.persistence.upsert_position(&closed_pos),
                                "Failed to persist closed position in close_position",
                                inner.persist_max_retries,
                                inner.persist_retry_base_ms
                            );
                            inner.emit_event(EngineEvent::PositionClosed {
                                position: closed_pos.clone(),
                            });
                            inner.emit_event(EngineEvent::PositionUpdated {
                                position: closed_pos,
                            });
                        }
                    }
                    None => {
                        error!(order_id = %order.id, "Close order filled but no valid fill_price — skipping Trade record to prevent 0.0 price propagation");
                    }
                }
            } else {
                inner.emit_event(EngineEvent::OrderPlaced { order });
                info!(position_id = %position_id, symbol = %position.symbol, "Close order placed");
            }

            let key = (
                position.exchange.clone(),
                position.symbol.clone(),
                position.side,
            );
            if let Some(mut pos) = inner.positions.get_mut(&key) {
                pos.status = PositionStatus::Closing;
                pos.updated_at = Utc::now();
            }
        }
        Ok(Err(e)) => {
            let msg = format!("Failed to place close order: {}", e);
            error!(error = %e, position_id = %position_id, "Failed to place close order");
            inner.emit_event(EngineEvent::OrderFailed {
                order_id: Uuid::nil(),
                reason: msg,
            });
        }
        Err(_elapsed) => {
            let msg = format!(
                "Close order timed out after {}s for position {}",
                close_order_timeout.as_secs(),
                position_id
            );
            warn!(position_id = %position_id, "{}", msg);
            inner.emit_event(EngineEvent::OrderFailed {
                order_id: Uuid::nil(),
                reason: msg,
            });
        }
    }
}

pub(crate) async fn handle_close_all_positions(inner: &Arc<EngineInner>, symbol: &str) {
    handle_cancel_all_orders(inner, None, Some(symbol.to_string())).await;

    let positions_to_close: Vec<(Uuid, PositionSide, f64)> = inner
        .positions
        .iter()
        .filter(|entry| {
            let pos = entry.value();
            pos.symbol == symbol && pos.size > 0.0
        })
        .map(|entry| {
            let pos = entry.value();
            (pos.id, pos.side, pos.size)
        })
        .collect();

    if positions_to_close.is_empty() {
        return;
    }

    info!(symbol = %symbol, count = positions_to_close.len(), "Closing all positions");

    for (position_id, _, _) in positions_to_close {
        handle_close_position(inner, position_id, OrderType::Market, None, None).await;
    }
}


pub(crate) fn resolve_position_side_for_hedge(params: &mut PlaceOrderParams) {
    if params.position_side.is_none() {
        params.position_side = match (&params.side, params.reduce_only) {
            (Side::Buy, false) => Some(PositionSide::Long),
            (Side::Sell, false) => Some(PositionSide::Short),
            (Side::Sell, true) => Some(PositionSide::Long),
            (Side::Buy, true) => Some(PositionSide::Short),
        };
    }
    params.reduce_only = false;
}

pub(crate) async fn handle_place_order(inner: &Arc<EngineInner>, mut params: PlaceOrderParams) {
    resolve_position_side_for_hedge(&mut params);


    let position_id = match params.position_id {
        Some(pid) => pid,
        None => {
            let pos_id = Uuid::new_v4();


            let position_side = params.position_side
                .expect("position_side must be resolved by resolve_position_side_for_hedge");
            let exchange_name = inner.exchange.name().to_string();
            let key = (exchange_name.clone(), params.symbol.clone(), position_side);


            if let Some(existing) = inner.positions.get(&key) {
                existing.id
            } else {
                let position = Position {
                    id: pos_id,
                    strategy_id: params.client_order_id.clone(),
                    exchange: exchange_name,
                    symbol: params.symbol.clone(),
                    side: position_side,
                    status: PositionStatus::Opening,
                    size: 0.0,
                    entry_price: 0.0,
                    current_price: 0.0,
                    leverage: 1,
                    margin: 0.0,
                    unrealized_pnl: 0.0,
                    realized_pnl: 0.0,
                    stop_loss: None,
                    take_profit: None,
                    liquidation_price: None,
                    opened_at: Utc::now(),
                    updated_at: Utc::now(),
                    closed_at: None,
                    metadata: serde_json::json!({}),
                };
                inner.position_id_index.insert(pos_id, key.clone());
                inner.positions.insert(key, position.clone());
                persist!(
                    inner.persistence.upsert_position(&position),
                    "Failed to persist auto-created position",
                    inner.persist_max_retries,
                    inner.persist_retry_base_ms
                );
                pos_id
            }
        }
    };
    params.position_id = Some(position_id);

    match inner.exchange.place_order(params.clone()).await {
        Ok(mut order) => {
            order.position_id = position_id;
            order.reduce_only = params.reduce_only;
            inner.orders.insert(order.id, order.clone());
            if let Some(ref eoid) = order.exchange_order_id {
                inner.exchange_order_id_index.insert(eoid.clone(), order.id);
            }
            if let Some(ref cid) = order.client_order_id {
                inner.client_order_id_index.insert(cid.clone(), order.id);
            }
            inner.emit_event(EngineEvent::OrderPlaced {
                order: order.clone(),
            });
        }
        Err(e) => {
            let msg = format!("Failed to place order: {}", e);
            error!(error = %e, symbol = %params.symbol, "Failed to place order");
            inner.emit_event(EngineEvent::OrderFailed {
                order_id: Uuid::nil(),
                reason: msg,
            });
        }
    }
}

pub(crate) async fn handle_cancel_order(inner: &Arc<EngineInner>, order_id: Uuid) {
    let order = inner.orders.get(&order_id).map(|r| r.value().clone());
    let order = match order {
        Some(o) => o,
        None => {
            let msg = format!("Order not found: {}", order_id);
            warn!(msg);
            inner.emit_event(EngineEvent::OrderFailed {
                order_id,
                reason: msg,
            });
            return;
        }
    };

    let exchange_order_id = match &order.exchange_order_id {
        Some(id) => id.clone(),
        None => {
            inner.emit_event(EngineEvent::OrderFailed {
                order_id,
                reason: format!("Order {} has no exchange_order_id", order_id),
            });
            return;
        }
    };

    match inner
        .exchange
        .cancel_order(&order.symbol, &exchange_order_id)
        .await
    {
        Ok(cancelled_order) => {
            inner
                .orders
                .insert(cancelled_order.id, cancelled_order.clone());
            inner.emit_event(EngineEvent::OrderCanceled {
                order: cancelled_order,
            });
        }
        Err(e) => {
            let msg = format!("Failed to cancel order: {}", e);
            error!(error = %e, order_id = %order_id, "Failed to cancel order");
            inner.emit_event(EngineEvent::OrderFailed {
                order_id,
                reason: msg,
            });
        }
    }
}

pub(crate) async fn handle_cancel_all_orders(
    inner: &Arc<EngineInner>,
    position_id: Option<Uuid>,
    symbol: Option<String>,
) {
    let target_symbol: Option<String> = if let Some(ref sym) = symbol {
        Some(sym.clone())
    } else if let Some(pid) = position_id {
        inner
            .positions
            .iter()
            .find(|r| r.value().id == pid)
            .map(|r| r.value().symbol.clone())
    } else {
        None
    };

    match inner
        .exchange
        .cancel_all_orders(target_symbol.as_deref())
        .await
    {
        Ok(cancelled_orders) => {
            for order in &cancelled_orders {
                inner.orders.insert(order.id, order.clone());
                inner.emit_event(EngineEvent::OrderCanceled {
                    order: order.clone(),
                });
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to cancel all orders");
        }
    }
}
