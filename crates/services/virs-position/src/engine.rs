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
use virs_types::CcxtOrder;
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
    pub(crate) orders: DashMap<String, CcxtOrder>,
    pub(crate) pending_orders: DashMap<String, PendingOrder>,
    pub(crate) order_position: DashMap<String, Uuid>,
    pub(crate) event_tx: broadcast::Sender<EngineEvent>,
    pub(crate) tracker: Mutex<PnlTracker>,
    pub(crate) state: RwLock<EngineState>,
    pub(crate) position_id_index: DashMap<Uuid, (String, String, PositionSide)>,
    #[allow(dead_code)]
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
            pending_orders: DashMap::new(),
            order_position: DashMap::new(),
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
            EngineCommand::CancelOrder { client_order_id } => {
                handle_cancel_order(&inner, client_order_id).await;
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
                    Some(WsFeedEvent::OrderUpdate { order }) => {
                        handle_ws_order_update(&inner, order).await;
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


pub(crate) async fn handle_ws_order_update(inner: &Arc<EngineInner>, ws_order: CcxtOrder) {
    let client_order_id = ws_order.client_order_id.clone();
    let order_status: OrderStatus = ws_order.status.clone().into();
    let filled: f64 = ws_order.filled_qty.parse().unwrap_or(0.0);
    let avg_price: f64 = ws_order.avg_fill_price.parse().unwrap_or(0.0);
    let commission: f64 = ws_order.commission.parse().unwrap_or(0.0);
    let realized_pnl: f64 = ws_order.realized_pnl.parse().unwrap_or(0.0);
    let is_reduce_only = ws_order.reduce_only
        || matches!(
            (&ws_order.side, &ws_order.position_side),
            (Side::Sell, PositionSide::Long) | (Side::Buy, PositionSide::Short)
        );
    let timestamp =
        chrono::DateTime::from_timestamp_millis(ws_order.trade_time).unwrap_or_else(Utc::now);

    // 1. 检查 pending_orders 中是否有此 client_order_id
    if let Some(mut pending) = inner.pending_orders.get_mut(&client_order_id) {
        pending.ws_order = Some(ws_order.clone());

        // 双确认：检查 rest_result 是否也存在
        if pending.rest_result.is_some() {
            let position_id = pending.position_id;
            drop(pending);
            finalize_pending_order(inner, &client_order_id, ws_order, position_id).await;
        }
        // REST 还没返回，WS 数据暂存 pending，等 REST 到达后再处理
        return;
    }

    // 2. 如果不在 pending，检查 orders 中是否已存在（后续 WS 更新）
    if let Some(mut existing) = inner.orders.get_mut(&client_order_id) {
        let prev_filled: f64 = existing.filled_qty.parse().unwrap_or(0.0);
        *existing = ws_order.clone();
        drop(existing);

        let trade_fill = filled - prev_filled;
        if trade_fill > 0.0
            && (order_status == OrderStatus::Filled
                || order_status == OrderStatus::PartiallyFilled)
        {
            let position_id = inner
                .order_position
                .get(&client_order_id)
                .map(|r| *r.value());
            process_order_fill(
                inner,
                &ws_order,
                &client_order_id,
                position_id,
                filled,
                trade_fill,
                avg_price,
                commission,
                realized_pnl,
                is_reduce_only,
                timestamp,
                order_status,
            )
            .await;
        }

        // 处理取消终态
        if order_status == OrderStatus::Canceled {
            if let Some((_, order)) = inner.orders.remove(&client_order_id) {
                inner.order_position.remove(&client_order_id);
                inner.emit_event(EngineEvent::OrderCanceled { order });
            }
        }
        return;
    }

    // 3. 既不在 pending 也不在 orders，忽略
    warn!(client_order_id = %client_order_id, "WS order update for unknown order, ignoring");
}


/// 双确认成功后，将订单从 pending 移入 orders，并处理后续事件（成交/取消/放置）。
async fn finalize_pending_order(
    inner: &Arc<EngineInner>,
    client_order_id: &str,
    ws_order: CcxtOrder,
    position_id: Option<Uuid>,
) {
    inner.pending_orders.remove(client_order_id);
    inner
        .orders
        .insert(client_order_id.to_string(), ws_order.clone());
    if let Some(pid) = position_id {
        inner
            .order_position
            .insert(client_order_id.to_string(), pid);
    }

    let order_status: OrderStatus = ws_order.status.clone().into();
    let filled: f64 = ws_order.filled_qty.parse().unwrap_or(0.0);
    let avg_price: f64 = ws_order.avg_fill_price.parse().unwrap_or(0.0);
    let commission: f64 = ws_order.commission.parse().unwrap_or(0.0);
    let realized_pnl: f64 = ws_order.realized_pnl.parse().unwrap_or(0.0);
    let is_reduce_only = ws_order.reduce_only
        || matches!(
            (&ws_order.side, &ws_order.position_side),
            (Side::Sell, PositionSide::Long) | (Side::Buy, PositionSide::Short)
        );
    let timestamp =
        chrono::DateTime::from_timestamp_millis(ws_order.trade_time).unwrap_or_else(Utc::now);

    if order_status == OrderStatus::Filled || order_status == OrderStatus::PartiallyFilled {
        if filled > 0.0 {
            // prev_filled = 0（首次确认）
            process_order_fill(
                inner,
                &ws_order,
                client_order_id,
                position_id,
                filled,
                filled,
                avg_price,
                commission,
                realized_pnl,
                is_reduce_only,
                timestamp,
                order_status,
            )
            .await;
        } else {
            inner.emit_event(EngineEvent::OrderPlaced { order: ws_order });
        }
    } else if order_status == OrderStatus::Canceled {
        inner.emit_event(EngineEvent::OrderCanceled {
            order: ws_order.clone(),
        });
        inner.orders.remove(client_order_id);
        inner.order_position.remove(client_order_id);
    } else {
        inner.emit_event(EngineEvent::OrderPlaced { order: ws_order });
    }
}


/// 处理订单成交：构造 Trade、更新仓位、emit 成交事件。
async fn process_order_fill(
    inner: &Arc<EngineInner>,
    ws_order: &CcxtOrder,
    client_order_id: &str,
    position_id: Option<Uuid>,
    filled: f64,
    trade_fill: f64,
    avg_price: f64,
    commission: f64,
    _realized_pnl: f64,
    is_reduce_only: bool,
    timestamp: chrono::DateTime<Utc>,
    order_status: OrderStatus,
) {
    let pos_key_opt = position_id.and_then(|pid| {
        inner
            .position_id_index
            .get(&pid)
            .map(|r| r.value().clone())
    });

    // 计算 pnl、trade_side、trade_type
    let (pnl, trade_side, trade_type) = match &pos_key_opt {
        Some(key) => {
            let pos_entry = inner.positions.get(key);
            match pos_entry {
                Some(pe) => {
                    let pos = pe.value();
                    if is_reduce_only {
                        let p = match pos.side {
                            PositionSide::Long => (avg_price - pos.entry_price) * trade_fill,
                            PositionSide::Short => (pos.entry_price - avg_price) * trade_fill,
                        };
                        let side = match pos.side {
                            PositionSide::Long => Side::Sell,
                            PositionSide::Short => Side::Buy,
                        };
                        (p, side, TradeType::Close)
                    } else {
                        let side = match pos.side {
                            PositionSide::Long => Side::Buy,
                            PositionSide::Short => Side::Sell,
                        };
                        (0.0, side, TradeType::Open)
                    }
                }
                None => (0.0, Side::Buy, TradeType::Open),
            }
        }
        None => (0.0, Side::Buy, TradeType::Open),
    };

    if avg_price <= 0.0 {
        error!(
            client_order_id = %client_order_id,
            symbol = %ws_order.symbol,
            price = avg_price,
            "WS order update has invalid price (<=0.0) — skipping Trade record to prevent 0.0 price propagation"
        );
        return;
    }

    let trade = Trade {
        id: Uuid::new_v4(),
        position_id: position_id.unwrap_or(Uuid::nil()),
        order_id: Uuid::nil(),
        exchange: inner.exchange.name().to_string(),
        symbol: ws_order.symbol.clone(),
        side: trade_side,
        price: avg_price,
        amount: trade_fill,
        fee: commission,
        fee_currency: ws_order.commission_asset.clone(),
        pnl,
        trade_type,
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

    match order_status {
        OrderStatus::Filled => {
            inner.emit_event(EngineEvent::OrderFilled {
                order: ws_order.clone(),
                trade: trade.clone(),
            });
        }
        OrderStatus::PartiallyFilled => {
            inner.emit_event(EngineEvent::OrderPartiallyFilled {
                order: ws_order.clone(),
                trade: trade.clone(),
            });
        }
        _ => {}
    }

    // 仓位更新（只在完全成交时）
    if order_status.is_filled() {
        if let Some(key) = &pos_key_opt {
            let pos_entry = inner.positions.get(key).map(|r| r.value().clone());
            if let Some(mut position) = pos_entry {
                if is_reduce_only {
                    position.size -= filled;
                    if position.size.abs() < 1e-8 {
                        position.size = 0.0;
                        position.status = PositionStatus::Closed;
                        position.closed_at = Some(timestamp);
                    } else {
                        position.status = PositionStatus::Open;
                    }
                } else {
                    let old_size = position.size;
                    position.size += filled;
                    if avg_price > 0.0 {
                        if old_size > 0.0 && position.entry_price > 0.0 {
                            let total_cost =
                                position.entry_price * old_size + avg_price * filled;
                            position.entry_price = total_cost / position.size;
                        } else {
                            position.entry_price = avg_price;
                        }
                        position.current_price = avg_price;
                    }
                    position.status = PositionStatus::Open;
                }
                position.updated_at = timestamp;
                let pos_clone = position.clone();

                if pos_clone.status == PositionStatus::Closed {
                    inner.position_id_index.remove(&pos_clone.id);
                    inner.positions.remove(key);
                    inner.emit_event(EngineEvent::PositionClosed {
                        position: pos_clone.clone(),
                    });
                    inner.emit_event(EngineEvent::PositionUpdated {
                        position: pos_clone.clone(),
                    });
                } else {
                    inner.positions.insert(key.clone(), pos_clone.clone());
                    inner.emit_event(EngineEvent::PositionUpdated {
                        position: pos_clone.clone(),
                    });
                }

                persist!(
                    inner.persistence.upsert_position(&pos_clone),
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

    // 如果仓位已存在，直接下单
    if let Some(existing) = inner.positions.get(&key) {
        let position_id = existing.id;
        drop(existing);

        let resolved_side = match side {
            PositionSide::Long => Side::Buy,
            PositionSide::Short => Side::Sell,
        };

        let params = PlaceOrderParams {
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
        handle_place_order(inner, params).await;
        return;
    }

    // 仓位不存在，创建新仓位
    if leverage == 0 {
        let msg = "leverage must be > 0".to_string();
        error!(symbol = %symbol, "open_position rejected: leverage is 0");
        let client_order_id = Uuid::new_v4().to_string();
        inner.emit_event(EngineEvent::OrderFailed {
            client_order_id,
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
        let client_order_id = Uuid::new_v4().to_string();
        inner.emit_event(EngineEvent::OrderFailed {
            client_order_id,
            reason: msg,
        });
        return;
    }

    let now = Utc::now();
    let position_id = Uuid::new_v4();
    let position = Position {
        id: position_id,
        strategy_id: strategy_id.clone(),
        exchange: exchange_name.clone(),
        symbol: symbol.clone(),
        side,
        status: PositionStatus::Opening,
        size: 0.0,
        entry_price: 0.0,
        current_price: 0.0,
        leverage,
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

    inner.position_id_index.insert(position.id, key.clone());
    inner.positions.insert(key, position.clone());
    persist!(
        inner.persistence.upsert_position(&position),
        "Failed to persist position in open_position",
        inner.persist_max_retries,
        inner.persist_retry_base_ms
    );
    inner.emit_event(EngineEvent::PositionOpened {
        position: position.clone(),
    });
    inner.emit_event(EngineEvent::PositionUpdated { position });

    let resolved_side = match side {
        PositionSide::Long => Side::Buy,
        PositionSide::Short => Side::Sell,
    };

    let params = PlaceOrderParams {
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
    handle_place_order(inner, params).await;
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
            let client_order_id = Uuid::new_v4().to_string();
            inner.emit_event(EngineEvent::OrderFailed {
                client_order_id,
                reason: msg,
            });
            return;
        }
    };

    if position.size == 0.0 {
        let client_order_id = Uuid::new_v4().to_string();
        inner.emit_event(EngineEvent::OrderFailed {
            client_order_id,
            reason: format!("Position {} has zero size", position_id),
        });
        return;
    }

    let close_side = match position.side {
        PositionSide::Long => Side::Sell,
        PositionSide::Short => Side::Buy,
    };

    let params = PlaceOrderParams {
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

    // 将仓位状态改为 Closing
    let key = (
        position.exchange.clone(),
        position.symbol.clone(),
        position.side,
    );
    if let Some(mut pos) = inner.positions.get_mut(&key) {
        pos.status = PositionStatus::Closing;
        pos.updated_at = Utc::now();
    }

    handle_place_order(inner, params).await;
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

    // 解析 position_id
    let position_id = match params.position_id {
        Some(pid) => pid,
        None => {
            let pos_id = Uuid::new_v4();
            let position_side = params
                .position_side
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

    // 生成 client_order_id
    let client_order_id = params
        .client_order_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    params.client_order_id = Some(client_order_id.clone());

    // Pre-register: 先存入 pending
    inner.pending_orders.insert(
        client_order_id.clone(),
        PendingOrder {
            client_order_id: client_order_id.clone(),
            params: params.clone(),
            rest_result: None,
            ws_order: None,
            position_id: Some(position_id),
            created_at: Utc::now(),
        },
    );

    // 调用 REST（params 在此处 move）
    let symbol_for_error = params.symbol.clone();
    match inner.exchange.place_order(params).await {
        Ok(result) => {
            // 填入 rest_result
            if let Some(mut pending) = inner.pending_orders.get_mut(&client_order_id) {
                pending.rest_result = Some(result);
                drop(pending);
            }

            // 检查 WS 是否已到达
            let ws_order = inner
                .pending_orders
                .get(&client_order_id)
                .and_then(|p| p.ws_order.clone());

            if let Some(ws_order) = ws_order {
                // 双确认成功，移除 pending，存入 orders
                finalize_pending_order(inner, &client_order_id, ws_order, Some(position_id))
                    .await;
            }
            // 如果 WS 未到达，等待 WS 回调处理
        }
        Err(e) => {
            inner.pending_orders.remove(&client_order_id);
            let msg = format!("Failed to place order: {}", e);
            error!(error = %e, symbol = %symbol_for_error, "Failed to place order");
            inner.emit_event(EngineEvent::OrderFailed {
                client_order_id,
                reason: msg,
            });
        }
    }
}


pub(crate) async fn handle_cancel_order(inner: &Arc<EngineInner>, client_order_id: String) {
    // 从 orders 中获取订单信息
    let (symbol, exchange_order_id) = {
        let order = inner.orders.get(&client_order_id);
        match order {
            Some(o) => (o.symbol.clone(), o.order_id.to_string()),
            None => {
                // 可能在 pending 中
                if let Some(pending) = inner.pending_orders.get(&client_order_id) {
                    if let Some(rest_result) = &pending.rest_result {
                        // 用 REST 返回的 order_id 撤单
                        (pending.params.symbol.clone(), rest_result.order_id.clone())
                    } else {
                        warn!(client_order_id = %client_order_id, "Cancel order: order not yet placed");
                        return;
                    }
                } else {
                    warn!(client_order_id = %client_order_id, "Cancel order: order not found");
                    return;
                }
            }
        }
    };

    match inner.exchange.cancel_order(&symbol, &exchange_order_id).await {
        Ok(_result) => {
            // WS 会推送 CANCELED 状态
            // 可以立即从 orders 中移除或等待 WS
        }
        Err(e) => {
            let msg = format!("Failed to cancel order: {}", e);
            error!(error = %e, client_order_id = %client_order_id, "Cancel order failed");
            inner.emit_event(EngineEvent::OrderFailed {
                client_order_id,
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
        Ok(results) => {
            for result in &results {
                let cid = &result.client_order_id;
                // 从 orders 中查找 CcxtOrder
                if let Some((_, order)) = inner.orders.remove(cid) {
                    inner.order_position.remove(cid);
                    inner.emit_event(EngineEvent::OrderCanceled { order });
                } else {
                    // 可能在 pending 中
                    inner.pending_orders.remove(cid);
                    // 没有 CcxtOrder，无法 emit OrderCanceled，等待 WS 推送
                }
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to cancel all orders");
        }
    }
}
