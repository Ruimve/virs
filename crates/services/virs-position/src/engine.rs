use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use futures_util::StreamExt;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use virs_task::{spawn, Stop, TaskHandle};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use virs_error::{VirsError, VirsResult};
use virs_type::*;
use virs_type::{
    CcxtOrder, CcxtOrderStatus, ExchangePe, ExchangePosition, ExecutionType, OrderUpdateStream,
    PositionEngineHandle,
};

use crate::persistence::PositionPersistence;

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
    ($expr:expr, $label:expr, $max_retries:expr, $base_ms:expr, $cancel:expr $(, $ctx_key:ident = $ctx_val:expr)* $(,)?) => {
        let cancel = $cancel;
        let mut attempts = 0u32;
        loop {
            match $expr.await {
                Ok(()) => break,
                Err(e) => {
                    attempts += 1;
                    if attempts >= $max_retries {
                        error!(error = %e, attempts $(, $ctx_key = %$ctx_val)*, $label);
                        break;
                    }
                    warn!(error = %e, attempt = attempts $(, $ctx_key = %$ctx_val)*, $label);
                    let sleep_dur = std::time::Duration::from_millis($base_ms * attempts as u64);
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            warn!(label = $label, "persist retry interrupted by cancellation, aborting persist");
                            return;
                        }
                        _ = tokio::time::sleep(sleep_dur) => {}
                    }
                }
            }
        }
    };
}

macro_rules! parse_field {
    ($expr:expr, $field:expr, $coid:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                error!(
                    client_order_id = %$coid,
                    field = $field,
                    error = %e,
                    "parse failed — skipping WS event to prevent default value propagation"
                );
                return;
            }
        }
    };
}

pub(crate) struct EngineInner {
    pub(crate) exchange: Arc<dyn ExchangePe>,
    pub(crate) persistence: Box<dyn PositionPersistence>,
    pub(crate) positions: DashMap<(String, String, PositionSide), Position>,
    pub(crate) orders: DashMap<String, Arc<CcxtOrder>>,
    pub(crate) pending_orders: DashMap<String, PendingOrder>,
    pub(crate) order_position: DashMap<String, Uuid>,
    pub(crate) event_tx: broadcast::Sender<EngineEvent>,
    pub(crate) state: RwLock<EngineState>,
    pub(crate) position_id_index: DashMap<Uuid, (String, String, PositionSide)>,
    pub(crate) persist_max_retries: u32,
    pub(crate) persist_retry_base_ms: u64,
    pub(crate) cancel: CancellationToken,
    pub(crate) run_task: Mutex<Option<TaskHandle>>,
}

impl EngineInner {
    fn emit_event(&self, event: EngineEvent) {
        if self.event_tx.receiver_count() > 0
            && self.event_tx.send(event).is_err() {
                debug!("EngineEvent broadcast — receiver dropped between check and send");
            }
    }

    fn rollback_position_on_order_terminal(&self, position_id: Uuid, context: &str) {
        let pos_key = self
            .position_id_index
            .get(&position_id)
            .map(|r| r.value().clone());
        if let Some(key) = pos_key {
            if let Some(mut pos) = self.positions.get_mut(&key) {
                if pos.is_ghost() {
                    let id = pos.id;
                    drop(pos);
                    self.positions.remove(&key);
                    self.position_id_index.remove(&id);
                    warn!(position_id = %position_id, context = %context, "Removed ghost Opening position after order terminal");
                } else if pos.status == PositionStatus::Closing {
                    pos.rollback_to_open(Utc::now());
                    let pos_clone = pos.clone();
                    drop(pos);
                    warn!(position_id = %position_id, context = %context, "Rolled back Closing position to Open after order terminal");
                    self.emit_event(EngineEvent::PositionUpdated {
                        position: pos_clone,
                    });
                }
            }
        }
    }

    fn set_state(&self, new_state: EngineState) {
        *recover_lock(self.state.write()) = new_state;
    }
}

pub(crate) struct PositionEngine {
    inner: Arc<EngineInner>,
    cmd_tx: mpsc::Sender<EngineCommand>,
    cmd_rx: Option<mpsc::Receiver<EngineCommand>>,
    cmd_loop_task: Option<TaskHandle>,
    ws_feed_loop_task: Option<TaskHandle>,
}

impl Clone for PositionEngine {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            cmd_tx: self.cmd_tx.clone(),
            cmd_rx: None,
            cmd_loop_task: None,
            ws_feed_loop_task: None,
        }
    }
}

impl PositionEngine {
    pub(crate) fn new(
        exchange: Arc<dyn ExchangePe>,
        persistence: Box<dyn PositionPersistence>,
        persist_max_retries: u32,
        persist_retry_base_ms: u64,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let event_tx = broadcast::channel(256).0;
        let cancel = CancellationToken::new();

        let inner = EngineInner {
            persistence,
            state: RwLock::new(EngineState::Created),
            exchange,
            event_tx,
            positions: DashMap::new(),
            orders: DashMap::new(),
            pending_orders: DashMap::new(),
            order_position: DashMap::new(),
            position_id_index: DashMap::new(),
            persist_max_retries,
            persist_retry_base_ms,
            cancel,
            run_task: Mutex::new(None),
        };

        Self {
            inner: Arc::new(inner),
            cmd_tx,
            cmd_rx: Some(cmd_rx),
            cmd_loop_task: None,
            ws_feed_loop_task: None,
        }
    }

    pub(crate) fn command_sender(&self) -> mpsc::Sender<EngineCommand> {
        self.cmd_tx.clone()
    }

    pub(crate) fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent> {
        self.inner.event_tx.subscribe()
    }

    pub(crate) fn event_sender(&self) -> broadcast::Sender<EngineEvent> {
        self.inner.event_tx.clone()
    }

    pub(crate) fn get_all_positions(&self) -> Vec<Position> {
        self.inner
            .positions
            .iter()
            .map(|r| r.value().clone())
            .collect()
    }

    pub(crate) fn get_open_positions_by_symbol(&self, symbol: &str) -> Vec<Position> {
        self.inner
            .positions
            .iter()
            .filter(|r| r.value().symbol == symbol && r.value().is_open())
            .map(|r| r.value().clone())
            .collect()
    }

    pub(crate) fn exchange(&self) -> Arc<dyn ExchangePe> {
        Arc::clone(&self.inner.exchange)
    }

    pub(crate) async fn run(&mut self) -> VirsResult<()> {
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

        let cmd_rx = self.cmd_rx.take().ok_or(VirsError::Http {
            status: 500,
            message: "Channel closed".to_string(),
        })?;
        let inner = Arc::clone(&self.inner);

        let (exit_tx, mut exit_rx) = mpsc::channel::<()>(2);

        self.cmd_loop_task = Some(spawn("position_command_loop", {
            let inner = inner.clone();
            let exit_tx = exit_tx.clone();
            move |task_stop| async move {
                command_loop(inner, cmd_rx, task_stop).await;
                let _ = exit_tx.try_send(());
            }
        }));

        self.ws_feed_loop_task = Some(spawn("position_ws_feed_loop", {
            let inner = inner.clone();
            let exit_tx = exit_tx;
            move |task_stop| async move {
                ws_feed_loop(inner, ws_feed_rx, task_stop).await;
                let _ = exit_tx.try_send(());
            }
        }));

        tokio::select! {
            _ = exit_rx.recv() => {}
            _ = inner.cancel.cancelled() => {}
        }

        self.inner.set_state(EngineState::ShuttingDown);
        inner.cancel.cancel();
        if let Some(h) = self.cmd_loop_task.take() {
            h.cancel();
            h.join().await;
        }
        if let Some(h) = self.ws_feed_loop_task.take() {
            h.cancel();
            h.join().await;
        }

        self.inner.set_state(EngineState::Stopped);
        info!("Position engine stopped");
        Ok(())
    }

    pub(crate) async fn stop(&self) {
        self.inner.set_state(EngineState::ShuttingDown);
        self.inner.cancel.cancel();
        info!("Position engine stop requested");
        let task = self.inner.run_task.lock().unwrap().take();
        if let Some(h) = &task {
            h.cancel();
        }
        if let Some(h) = task {
            h.join().await;
        }
    }

    async fn recover_state(&self) -> VirsResult<()> {
        let exchange_name = self.inner.exchange.name().to_string();

        let positions = self
            .inner
            .persistence
            .get_positions_from_orders(&exchange_name)
            .await?;
        for pos in &positions {
            let key = (pos.exchange.clone(), pos.symbol.clone(), pos.side.clone());
            self.inner.position_id_index.insert(pos.id, key.clone());
            self.inner.positions.insert(key, pos.clone());
        }

        let active_orders = self.inner.persistence.get_active_orders().await?;
        for order in &active_orders {
            let pos_id = position_uuid_v5(&exchange_name, &order.symbol, &order.position_side);
            self.inner
                .order_position
                .insert(order.client_order_id.clone(), pos_id);
            self.inner
                .orders
                .insert(order.client_order_id.clone(), Arc::new(order.clone()));

            let is_open_order = matches!(
                (&order.side, &order.position_side),
                (Side::Buy, PositionSide::Long) | (Side::Sell, PositionSide::Short)
            );
            if is_open_order {
                let key = (
                    exchange_name.clone(),
                    order.symbol.clone(),
                    order.position_side.clone(),
                );
                if !self.inner.positions.contains_key(&key) {
                    let position = Position::new_opening(
                        &exchange_name,
                        &order.symbol,
                        order.position_side.clone(),
                        Some(order.client_order_id.clone()),
                    );
                    self.inner.position_id_index.insert(pos_id, key.clone());
                    self.inner.positions.insert(key, position);
                }
            }
        }

        let exchange_positions: Vec<ExchangePosition> = positions
            .iter()
            .map(|p| ExchangePosition {
                symbol: p.symbol.clone(),
                side: p.side.clone(),
                quantity: p.quantity,
                entry_price: p.entry_price,
                margin_mode: MarginMode::Cross,
                info: serde_json::Value::Null,
            })
            .collect();
        self.inner
            .exchange
            .restore_positions(exchange_positions)
            .await;

        Ok(())
    }
}

#[async_trait]
impl PositionEngineHandle for PositionEngine {
    fn command_sender(&self) -> mpsc::Sender<EngineCommand> {
        PositionEngine::command_sender(self)
    }

    fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent> {
        PositionEngine::subscribe_events(self)
    }

    fn event_sender(&self) -> broadcast::Sender<EngineEvent> {
        PositionEngine::event_sender(self)
    }

    fn exchange(&self) -> Arc<dyn ExchangePe> {
        PositionEngine::exchange(self)
    }

    fn get_all_positions(&self) -> Vec<Position> {
        PositionEngine::get_all_positions(self)
    }

    fn get_open_positions_by_symbol(&self, symbol: &str) -> Vec<Position> {
        PositionEngine::get_open_positions_by_symbol(self, symbol)
    }

    async fn stop(&self) {
        PositionEngine::stop(self).await
    }
}

/// 工厂函数：创建 PositionEngine，启动运行循环，并返回 trait 对象。
///
/// `run()` 需要 `&mut self`，无法通过 trait 对象调用，
/// 因此在工厂函数内部 spawn 后将 `TaskHandle` 存入 `EngineInner`（共享 Arc），
/// 外部仅通过 `Arc<dyn PositionEngineHandle>` 交互，`stop()` 内部完成 cancel + join。
pub fn create_position_engine(
    exchange: Arc<dyn ExchangePe>,
    persistence: Box<dyn PositionPersistence>,
    persist_max_retries: u32,
    persist_retry_base_ms: u64,
) -> Arc<dyn PositionEngineHandle> {
    let mut engine = PositionEngine::new(exchange, persistence, persist_max_retries, persist_retry_base_ms);
    let handle = Arc::new(engine.clone());
    let task = spawn("position_engine", move |_| async move {
        if let Err(e) = engine.run().await {
            error!(error = %e, "Position Engine run failed");
        }
    });
    *handle.inner.run_task.lock().unwrap() = Some(task);
    handle
}

pub(crate) async fn command_loop(
    inner: Arc<EngineInner>,
    mut cmd_rx: mpsc::Receiver<EngineCommand>,
    stop: Stop,
) {
    loop {
        tokio::select! {
            _ = stop.cancelled() => break,
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                match cmd {
                    EngineCommand::OpenPosition {
                        exchange,
                        symbol,
                        side,
                        order_side,
                        quantity,
                        leverage,
                        order_type,
                        price,
                        client_order_id,
                    } => {
                        handle_open_position(
                            &inner,
                            exchange,
                            symbol,
                            side,
                            order_side,
                            quantity,
                            leverage,
                            order_type,
                            price,
                            client_order_id,
                        )
                        .await;
                    }
                    EngineCommand::ClosePosition {
                        position_id,
                        order_type,
                        price,
                        client_order_id,
                    } => {
                        handle_close_position(&inner, position_id, order_type, price, client_order_id)
                            .await;
                    }
                    EngineCommand::PlaceOrder { params } => {
                        handle_place_order(&inner, params).await;
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
    }
}

pub(crate) async fn ws_feed_loop(
    inner: Arc<EngineInner>,
    mut ws_rx: OrderUpdateStream,
    stop: Stop,
) {
    loop {
        tokio::select! {
            _ = stop.cancelled() => break,
            event = ws_rx.next() => {
                match event {
                    Some(WsFeedEvent::OrderUpdate { order }) => {
                        handle_ws_order_update(&inner, order).await;
                    }
                    Some(WsFeedEvent::ConnectionChanged { .. }) => {}
                    None => break,
                }
            }
        }
    }
}

pub(crate) async fn handle_ws_order_update(inner: &Arc<EngineInner>, ws_order: Arc<CcxtOrder>) {
    let rejection_reason = match (&ws_order.side, &ws_order.position_side, &ws_order.status) {
        (Side::Unknown(raw), _, _) => Some(format!("InvalidSide({})", raw)),
        (_, PositionSide::Unknown(raw), _) => Some(format!("InvalidPositionSide({})", raw)),
        (_, _, CcxtOrderStatus::Unknown(raw)) => Some(format!("InvalidStatus({})", raw)),
        _ => None,
    };

    if let Some(reason) = rejection_reason {
        error!(
            symbol = %ws_order.symbol,
            client_order_id = %ws_order.client_order_id,
            order_id = ws_order.order_id,
            reason = %reason,
            "WS 订单字段校验失败，持久化到 pe_rejected_orders 并跳过业务处理"
        );
        persist!(
            inner.persistence.persist_rejected_order(&ws_order, &reason),
            "persist_rejected_order",
            inner.persist_max_retries,
            inner.persist_retry_base_ms,
            inner.cancel.clone(),
            symbol = ws_order.symbol,
            client_order_id = ws_order.client_order_id
        );
        return;
    }

    let client_order_id = ws_order.client_order_id.clone();
    let order_status: OrderStatus = ws_order.status.clone().into();
    let filled: f64 = parse_field!(ws_order.filled_qty.parse(), "filled_qty", client_order_id);
    let fill_price: f64 = parse_field!(ws_order.last_fill_price.parse(), "last_fill_price", client_order_id);
    let commission: f64 = parse_field!(ws_order.commission.parse(), "commission", client_order_id);
    let realized_pnl: f64 = parse_field!(ws_order.realized_pnl.parse(), "realized_pnl", client_order_id);
    let is_close = matches!(
        (&ws_order.side, &ws_order.position_side),
        (Side::Sell, PositionSide::Long) | (Side::Buy, PositionSide::Short)
    );
    let timestamp = match chrono::DateTime::from_timestamp_millis(ws_order.trade_time) {
        Some(ts) => ts,
        None => {
            error!(
                symbol = %ws_order.symbol,
                client_order_id = %ws_order.client_order_id,
                trade_time = ws_order.trade_time,
                "Invalid trade_time timestamp — skipping order"
            );
            return;
        }
    };

    if let Some(mut pending) = inner.pending_orders.get_mut(&client_order_id) {
        persist!(
            inner.persistence.persist_order(&ws_order),
            "persist_order (pending)",
            inner.persist_max_retries,
            inner.persist_retry_base_ms,
            inner.cancel.clone(),
            symbol = ws_order.symbol,
            client_order_id = ws_order.client_order_id
        );

        let position_id = pending.position_id;
        let rest_ready = pending.rest_result.is_some();
        pending.ws_order = Some(ws_order.clone());
        drop(pending);

        if ws_order.execution_type == ExecutionType::Trade
            && (order_status == OrderStatus::Filled
                || order_status == OrderStatus::PartiallyFilled)
        {
            let trade_fill: f64 = parse_field!(
                ws_order.last_fill_qty.parse(),
                "last_fill_qty",
                client_order_id
            );
            if trade_fill > 0.0 {
                process_order_fill(
                    inner,
                    &ws_order,
                    &client_order_id,
                    position_id,
                    trade_fill,
                    fill_price,
                    commission,
                    realized_pnl,
                    is_close,
                    timestamp,
                    order_status,
                )
                .await;
            }
        }

        if rest_ready {
            finalize_pending_order(inner, &client_order_id, ws_order, position_id).await;
        }
        return;
    }

    if let Some(mut existing) = inner.orders.get_mut(&client_order_id) {
        let prev_filled: f64 = parse_field!(
            existing.filled_qty.parse(),
            "prev_filled_qty",
            client_order_id
        );
        *existing = ws_order.clone();
        drop(existing);

        persist!(
            inner.persistence.persist_order(&ws_order),
            "persist_order",
            inner.persist_max_retries,
            inner.persist_retry_base_ms,
            inner.cancel.clone(),
            symbol = ws_order.symbol,
            client_order_id = ws_order.client_order_id
        );

        let trade_fill = filled - prev_filled;
        if trade_fill > 0.0
            && (order_status == OrderStatus::Filled || order_status == OrderStatus::PartiallyFilled)
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
                trade_fill,
                fill_price,
                commission,
                realized_pnl,
                is_close,
                timestamp,
                order_status,
            )
            .await;
        }

        if order_status == OrderStatus::Filled {
            inner.orders.remove(&client_order_id);
            inner.order_position.remove(&client_order_id);
        }

        if order_status == OrderStatus::Canceled || order_status == OrderStatus::Expired {
            let pos_id = inner.order_position.get(&client_order_id).map(|r| *r.value());
            if let Some((_, order)) = inner.orders.remove(&client_order_id) {
                inner.order_position.remove(&client_order_id);
                inner.emit_event(EngineEvent::OrderCanceled { order });
            }
            if let Some(pid) = pos_id {
                inner.rollback_position_on_order_terminal(pid, "ws_canceled");
            }
        }
        return;
    }

    warn!(client_order_id = %client_order_id, "WS order update for unknown order, ignoring");
}

async fn finalize_pending_order(
    inner: &Arc<EngineInner>,
    client_order_id: &str,
    ws_order: Arc<CcxtOrder>,
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

    persist!(
        inner.persistence.persist_order(&ws_order),
        "persist_order",
        inner.persist_max_retries,
        inner.persist_retry_base_ms,
        inner.cancel.clone(),
        symbol = ws_order.symbol,
        client_order_id = ws_order.client_order_id
    );

    let order_status: OrderStatus = ws_order.status.clone().into();

    if order_status == OrderStatus::Filled {
        inner.orders.remove(client_order_id);
        inner.order_position.remove(client_order_id);
    } else if order_status == OrderStatus::Canceled || order_status == OrderStatus::Expired {
        inner.emit_event(EngineEvent::OrderCanceled {
            order: ws_order.clone(),
        });
        inner.orders.remove(client_order_id);
        inner.order_position.remove(client_order_id);
        if let Some(pid) = position_id {
            inner.rollback_position_on_order_terminal(pid, "finalize_canceled");
        }
    } else if ws_order.execution_type == ExecutionType::New {
        inner.emit_event(EngineEvent::OrderPlaced { order: ws_order });
    }
}

async fn process_order_fill(
    inner: &Arc<EngineInner>,
    ws_order: &Arc<CcxtOrder>,
    client_order_id: &str,
    position_id: Option<Uuid>,
    trade_fill: f64,
    fill_price: f64,
    commission: f64,
    realized_pnl: f64,
    is_close: bool,
    timestamp: chrono::DateTime<Utc>,
    order_status: OrderStatus,
) {
    let pos_key_opt =
        position_id.and_then(|pid| inner.position_id_index.get(&pid).map(|r| r.value().clone()));

    let (pnl, trade_side, trade_type) = match &pos_key_opt {
        Some(key) => {
            let pos_entry = inner.positions.get(key);
            match pos_entry {
                Some(pe) => {
                    let pos = pe.value();
                    if is_close {
                        let side = match pos.side {
                            PositionSide::Long => Side::Sell,
                            PositionSide::Short => Side::Buy,
                            PositionSide::Unknown(_) => {
                                unreachable!("validate ensures position side is Long/Short")
                            }
                        };
                        (realized_pnl, side, TradeType::Close)
                    } else {
                        let side = match pos.side {
                            PositionSide::Long => Side::Buy,
                            PositionSide::Short => Side::Sell,
                            PositionSide::Unknown(_) => {
                                unreachable!("validate ensures position side is Long/Short")
                            }
                        };
                        (0.0, side, TradeType::Open)
                    }
                }
                None => (0.0, Side::Buy, TradeType::Open),
            }
        }
        None => (0.0, Side::Buy, TradeType::Open),
    };

    let skip_trade = fill_price <= 0.0;
    if skip_trade {
        error!(
            client_order_id = %client_order_id,
            symbol = %ws_order.symbol,
            price = fill_price,
            "WS order update has invalid price (<=0.0) — skipping Trade record but still updating position"
        );
    }

    if !skip_trade {
        let trade_position_id = position_id.unwrap_or_else(|| {
            warn!(
                client_order_id = %client_order_id,
                symbol = %ws_order.symbol,
                "Trade has no position_id — writing with Uuid::nil() (orphan trade record)"
            );
            Uuid::nil()
        });
        let trade = Trade {
            id: Uuid::new_v4(),
            position_id: trade_position_id,
            order_id: Uuid::nil(),
            exchange: inner.exchange.name().to_string(),
            symbol: ws_order.symbol.clone(),
            side: trade_side,
            price: fill_price,
            amount: trade_fill,
            fee: commission,
            fee_currency: ws_order.commission_asset.clone(),
            pnl,
            trade_type,
            created_at: timestamp,
        };

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
    }

    if let Some(key) = &pos_key_opt {
        if let Some(mut pos) = inner.positions.get_mut(key) {
            let is_closed = pos.apply_fill(is_close, fill_price, trade_fill, pnl, timestamp);
            let pos_clone = pos.clone();
            drop(pos);

            if is_closed {
                inner.position_id_index.remove(&pos_clone.id);
                inner.positions.remove(key);
                inner.emit_event(EngineEvent::PositionClosed {
                    position: pos_clone.clone(),
                });
                inner.emit_event(EngineEvent::PositionUpdated {
                    position: pos_clone,
                });
            } else {
                inner.emit_event(EngineEvent::PositionUpdated {
                    position: pos_clone,
                });
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
    quantity: f64,
    leverage: u32,
    order_type: OrderType,
    price: Option<f64>,
    client_order_id: Option<String>,
) {
    let exchange_name = if exchange.is_empty() {
        inner.exchange.name().to_string()
    } else {
        exchange
    };
    let key = (exchange_name.clone(), symbol.clone(), side.clone());

    if let Some(existing) = inner.positions.get(&key) {
        let position_id = existing.id;
        drop(existing);

        let resolved_side = match &side {
            PositionSide::Long => Side::Buy,
            PositionSide::Short => Side::Sell,
            PositionSide::Unknown(_) => {
                unreachable!("validate ensures position_side is Long/Short at WS entry")
            }
        };

        let params = PlaceOrderParams {
            symbol: symbol.clone(),
            side: resolved_side,
            order_type,
            amount: quantity,
            price,
            position_side: Some(side),
            position_id: Some(position_id),
            client_order_id: client_order_id.clone(),
            stop_price: None,
            time_in_force: Some(TimeInForce::Gtc),
        };
        handle_place_order(inner, params).await;
        return;
    }

    if leverage == 0 {
        let msg = "leverage must be > 0".to_string();
        error!(symbol = %symbol, "open_position rejected: leverage is 0");
        inner.emit_event(EngineEvent::OrderFailed {
            client_order_id: client_order_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
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
        let client_order_id = client_order_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        inner.emit_event(EngineEvent::OrderFailed {
            client_order_id,
            reason: msg,
        });
        return;
    }

    let position_id = position_uuid_v5(&exchange_name, &symbol, &side);
    let position = Position::new_opening(
        &exchange_name,
        &symbol,
        side.clone(),
        client_order_id.clone(),
    );

    inner.position_id_index.insert(position.id, key.clone());
    inner.positions.insert(key, position.clone());
    inner.emit_event(EngineEvent::PositionOpened {
        position: position.clone(),
    });
    inner.emit_event(EngineEvent::PositionUpdated { position });

    let resolved_side = match &side {
        PositionSide::Long => Side::Buy,
        PositionSide::Short => Side::Sell,
        PositionSide::Unknown(_) => {
            unreachable!("validate ensures position_side is Long/Short at WS entry")
        }
    };

    let params = PlaceOrderParams {
        symbol: symbol.clone(),
        side: resolved_side,
        order_type,
        amount: quantity,
        price,
        position_side: Some(side),
        position_id: Some(position_id),
        client_order_id: client_order_id.clone(),
        stop_price: None,
        time_in_force: Some(TimeInForce::Gtc),
    };
    handle_place_order(inner, params).await;
}

pub(crate) async fn handle_close_position(
    inner: &Arc<EngineInner>,
    position_id: Uuid,
    order_type: OrderType,
    price: Option<f64>,
    client_order_id: Option<String>,
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
            warn!(position_id = %position_id, "Position not found");
            let msg = format!("Position not found: {}", position_id);
            inner.emit_event(EngineEvent::OrderFailed {
                client_order_id: client_order_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
                reason: msg,
            });
            return;
        }
    };

    if position.quantity == 0.0 {
        inner.emit_event(EngineEvent::OrderFailed {
            client_order_id: client_order_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            reason: format!("Position {} has zero quantity", position_id),
        });
        return;
    }

    let close_side = match &position.side {
        PositionSide::Long => Side::Sell,
        PositionSide::Short => Side::Buy,
        PositionSide::Unknown(_) => {
            unreachable!("validate ensures position_side is Long/Short")
        }
    };

    let params = PlaceOrderParams {
        symbol: position.symbol.clone(),
        side: close_side,
        order_type,
        amount: position.quantity,
        price,
        position_side: Some(position.side.clone()),
        position_id: Some(position.id),
        client_order_id: client_order_id.clone(),
        stop_price: None,
        time_in_force: Some(TimeInForce::Gtc),
    };

    let key = (
        position.exchange.clone(),
        position.symbol.clone(),
        position.side.clone(),
    );
    if let Some(mut pos) = inner.positions.get_mut(&key) {
        pos.set_closing(Utc::now());
        let pos_clone = pos.clone();
        drop(pos);
        inner.emit_event(EngineEvent::PositionUpdated {
            position: pos_clone,
        });
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
            pos.symbol == symbol && pos.quantity > 0.0
        })
        .map(|entry| {
            let pos = entry.value();
            (pos.id, pos.side.clone(), pos.quantity)
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
        params.position_side = match &params.side {
            Side::Buy => Some(PositionSide::Long),
            Side::Sell => Some(PositionSide::Short),
            Side::Unknown(_) => {
                unreachable!("validate ensures side is Buy/Sell at WS entry")
            }
        };
    }
}

pub(crate) async fn handle_place_order(inner: &Arc<EngineInner>, mut params: PlaceOrderParams) {
    resolve_position_side_for_hedge(&mut params);

    let position_id = match params.position_id {
        Some(pid) => pid,
        None => {
            let position_side = match params.position_side.take() {
                Some(ps) => ps,
                None => {
                    error!(
                        client_order_id = %params.client_order_id.as_deref().unwrap_or("none"),
                        symbol = %params.symbol,
                        "position_side unresolved (side is Unknown), cannot place order"
                    );
                    let cid = match params.client_order_id.clone() {
                        Some(cid) => cid,
                        None => {
                            error!(
                                symbol = %params.symbol,
                                "client_order_id is None in OrderFailed path — \
                                 cannot emit OrderFailed event without client_order_id"
                            );
                            return;
                        }
                    };
                    inner.emit_event(EngineEvent::OrderFailed {
                        client_order_id: cid,
                        reason: "position_side unresolved (side is Unknown)".into(),
                    });
                    return;
                }
            };
            let exchange_name = inner.exchange.name().to_string();
            let pos_id = position_uuid_v5(&exchange_name, &params.symbol, &position_side);
            let key = (exchange_name.clone(), params.symbol.clone(), position_side.clone());

            if let Some(existing) = inner.positions.get(&key) {
                existing.id
            } else {
                let position = Position::new_opening(
                    &exchange_name,
                    &params.symbol,
                    position_side,
                    params.client_order_id.clone(),
                );
                inner.position_id_index.insert(pos_id, key.clone());
                inner.positions.insert(key, position);
                pos_id
            }
        }
    };
    params.position_id = Some(position_id);

    let client_order_id = match params.client_order_id.clone() {
        Some(cid) => cid,
        None => {
            error!(
                symbol = %params.symbol,
                "client_order_id is required for order placement — rolling back ghost position and emitting OrderFailed"
            );

            inner.rollback_position_on_order_terminal(position_id, "place_order_no_client_order_id");

            inner.emit_event(EngineEvent::OrderFailed {
                client_order_id: Uuid::new_v4().to_string(),
                reason: "client_order_id is required for order placement but was None".into(),
            });
            return;
        }
    };
    params.client_order_id = Some(client_order_id.clone());

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

    let symbol_for_error = params.symbol.clone();
    match inner.exchange.place_order(params).await {
        Ok(result) => {
            if let Some(mut pending) = inner.pending_orders.get_mut(&client_order_id) {
                pending.rest_result = Some(result);
                drop(pending);
            }

            let ws_order = inner
                .pending_orders
                .get(&client_order_id)
                .and_then(|p| p.ws_order.clone());

            if let Some(ws_order) = ws_order {
                finalize_pending_order(inner, &client_order_id, ws_order, Some(position_id)).await;
            }
        }
        Err(e) => {
            inner.pending_orders.remove(&client_order_id);
            let msg = format!("Failed to place order: {}", e);
            error!(error = %e, symbol = %symbol_for_error, "Failed to place order");

            inner.rollback_position_on_order_terminal(position_id, "place_order_rest_failure");

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
                let pos_id = inner.order_position.get(cid).map(|r| *r.value());
                if let Some((_, order)) = inner.orders.remove(cid) {
                    inner.order_position.remove(cid);
                    inner.emit_event(EngineEvent::OrderCanceled { order });
                } else {
                    inner.pending_orders.remove(cid);
                }
                if let Some(pid) = pos_id {
                    inner.rollback_position_on_order_terminal(pid, "cancel_all_orders");
                }
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to cancel all orders");
        }
    }
}
