use std::sync::{Arc, RwLock};

use chrono::Utc;
use dashmap::DashMap;
use futures_util::StreamExt;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};
use uuid::Uuid;

use virs_error::{VirsError, VirsResult};
use virs_types::enums::*;
use virs_types::exchange_pe::{ExchangePe, OrderUpdateStream};
use virs_types::market::ExchangePosition;
use virs_types::position::*;
use virs_types::{CcxtOrder, CcxtOrderStatus, ExecutionType};

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

/// 解析 String 字段为 f64，失败时记录 error 并从当前函数 return（跳过该 WS 事件）。
/// 用于关键交易参数（filled_qty、last_fill_price、commission 等），不允许默认值。
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

/// 解析 Option<String> 字段为 f64。
/// None → 0.0（语义正确：开仓单 rp=0、NEW 状态 avg_fill_price 无意义）
/// Some(s) 解析失败 → error + return（不允许默认值）
macro_rules! parse_opt_field {
    ($opt:expr, $field:expr, $coid:expr) => {
        match $opt.as_deref() {
            None => 0.0,
            Some(s) => match s.parse::<f64>() {
                Ok(v) => v,
                Err(e) => {
                    error!(
                        client_order_id = %$coid,
                        field = $field,
                        raw_value = s,
                        error = %e,
                        "parse failed — skipping WS event to prevent default value propagation"
                    );
                    return;
                }
            },
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
    pub(crate) state: RwLock<EngineState>,
    pub(crate) position_id_index: DashMap<Uuid, (String, String, PositionSide)>,
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
        persist_max_retries: u32,
        persist_retry_base_ms: u64,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let event_tx = broadcast::channel(256).0;
        let exchange: Arc<dyn ExchangePe> = Arc::from(exchange);

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

    /// 查询指定 symbol 下所有 open 仓位（Hedge 模式下可能同时返回 Long 和 Short）。
    pub fn get_open_positions_by_symbol(&self, symbol: &str) -> Vec<Position> {
        self.inner
            .positions
            .iter()
            .filter(|r| r.value().symbol == symbol && r.value().is_open())
            .map(|r| r.value().clone())
            .collect()
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

        let cmd_rx = self.cmd_rx.take().ok_or(VirsError::Http {
            status: 500,
            message: "Channel closed".to_string(),
        })?;
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
        })
        .await;

        self.inner.set_state(EngineState::Stopped);
        info!("Position engine stopped");
        Ok(())
    }

    pub fn stop(&self) {
        self.inner.set_state(EngineState::ShuttingDown);
        info!("Position engine stop requested");
    }

    async fn recover_state(&self) -> VirsResult<()> {
        let exchange_name = self.inner.exchange.name().to_string();

        // 1. 从 pe_trades 聚合恢复仓位
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

        // 2. 恢复活跃订单和 order_position 映射
        let active_orders = self.inner.persistence.get_active_orders().await?;
        for order in &active_orders {
            let pos_id = position_uuid_v5(&exchange_name, &order.symbol, &order.position_side);
            self.inner
                .order_position
                .insert(order.client_order_id.clone(), pos_id);
            self.inner
                .orders
                .insert(order.client_order_id.clone(), order.clone());

            // NEW 开仓订单（无成交）对应的 Opening 仓位不会被聚合 SQL 恢复
            // 需手动创建，否则 WS 成交时 position_id_index 查不到 key，仓位更新被跳过
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

        // 3. 恢复仓位到交易所适配器
        let exchange_positions: Vec<ExchangePosition> = positions
            .iter()
            .map(|p| ExchangePosition {
                symbol: p.symbol.clone(),
                side: p.side.clone(),
                quantity: p.quantity,
                entry_price: p.entry_price,
            })
            .collect();
        self.inner
            .exchange
            .restore_positions(exchange_positions)
            .await;

        Ok(())
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
    // 校验: side/position_side/status 是否合法 (Unknown 变体表示非法原始值)
    // 非法订单持久化到 pe_rejected_orders 并跳过业务处理
    let rejection_reason = match (&ws_order.side, &ws_order.position_side, &ws_order.status) {
        (Side::Unknown(raw), _, _) => Some(format!("InvalidSide({})", raw)),
        (_, PositionSide::Unknown(raw), _) => Some(format!("InvalidPositionSide({})", raw)),
        (_, _, CcxtOrderStatus::Unknown(raw)) => Some(format!("InvalidStatus({})", raw)),
        _ => None,
    };

    if let Some(reason) = rejection_reason {
        tracing::error!(
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
            inner.persist_retry_base_ms
        );
        return;
    }

    let client_order_id = ws_order.client_order_id.clone();
    let order_status: OrderStatus = ws_order.status.clone().into();
    let filled: f64 = parse_field!(ws_order.filled_qty.parse(), "filled_qty", client_order_id);
    // 路径2（增量更新）: 每个 WS 事件代表一笔成交, trade_fill 是增量量,
    // 必须用 last_fill_price(本笔成交价) 而非 avg_fill_price(累计均价) 做边际成本
    let fill_price: f64 = parse_field!(ws_order.last_fill_price.parse(), "last_fill_price", client_order_id);
    let commission: f64 = parse_field!(ws_order.commission.parse(), "commission", client_order_id);
    let realized_pnl: f64 = parse_opt_field!(ws_order.realized_pnl, "realized_pnl", client_order_id);
    // Hedge 模式下开平仓由 side + position_side 组合判断
    let is_close = matches!(
        (&ws_order.side, &ws_order.position_side),
        (Side::Sell, PositionSide::Long) | (Side::Buy, PositionSide::Short)
    );
    let timestamp =
        chrono::DateTime::from_timestamp_millis(ws_order.trade_time).unwrap_or_else(Utc::now);

    // 1. 检查 pending_orders 中是否有此 client_order_id
    if let Some(mut pending) = inner.pending_orders.get_mut(&client_order_id) {
        // 每笔 WS 事件立即持久化到 DB（每笔事件独立一行，不能只存内存）
        persist!(
            inner.persistence.persist_order(&ws_order),
            "persist_order (pending)",
            inner.persist_max_retries,
            inner.persist_retry_base_ms
        );

        let position_id = pending.position_id;
        let rest_ready = pending.rest_result.is_some();
        pending.ws_order = Some(ws_order.clone());
        drop(pending);

        // TRADE 事件且有成交：立即用增量字段处理 fill
        // 使用 last_fill_qty（本笔量）和 last_fill_price（本笔价），
        // 与 path 2（运行时增量）和 DB 回放逻辑完全一致
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

        // 双确认：检查 rest_result 是否也存在
        if rest_ready {
            finalize_pending_order(inner, &client_order_id, ws_order, position_id).await;
        }
        // REST 还没返回，WS 数据暂存 pending，等 REST 到达后再处理
        return;
    }

    // 2. 如果不在 pending，检查 orders 中是否已存在（后续 WS 更新）
    if let Some(mut existing) = inner.orders.get_mut(&client_order_id) {
        let prev_filled: f64 = parse_field!(
            existing.filled_qty.parse(),
            "prev_filled_qty",
            client_order_id
        );
        *existing = ws_order.clone();
        drop(existing);

        // 持久化订单更新到 DB
        persist!(
            inner.persistence.persist_order(&ws_order),
            "persist_order",
            inner.persist_max_retries,
            inner.persist_retry_base_ms
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

        // Filled 终态清理：即使无新增成交（重复推送），也需清理避免泄漏
        if order_status == OrderStatus::Filled {
            inner.orders.remove(&client_order_id);
            inner.order_position.remove(&client_order_id);
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

/// 双确认成功后，将订单从 pending 移入 orders，并处理终态（成交处理已在 pending 路径完成）。
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

    // 持久化订单到 DB（幂等：pending 路径已持久化，ON CONFLICT DO NOTHING 保证不重复）
    persist!(
        inner.persistence.persist_order(&ws_order),
        "persist_order",
        inner.persist_max_retries,
        inner.persist_retry_base_ms
    );

    let order_status: OrderStatus = ws_order.status.clone().into();

    // 成交处理已在 pending 路径完成（process_order_fill 用 last_fill_qty/last_fill_price），
    // 此处只处理订单终态清理和事件发射
    if order_status == OrderStatus::Filled {
        // Filled 终态清理
        inner.orders.remove(client_order_id);
        inner.order_position.remove(client_order_id);
    } else if order_status == OrderStatus::Canceled {
        inner.emit_event(EngineEvent::OrderCanceled {
            order: ws_order.clone(),
        });
        inner.orders.remove(client_order_id);
        inner.order_position.remove(client_order_id);
    } else if ws_order.execution_type == ExecutionType::New {
        // NEW 事件：emit OrderPlaced
        inner.emit_event(EngineEvent::OrderPlaced { order: ws_order });
    }
    // PartiallyFilled / TRADE：fill 事件已在 pending 路径 emit，订单留在 orders 中等待后续更新
}

/// 处理订单成交：构造 Trade、更新仓位、emit 成交事件。
async fn process_order_fill(
    inner: &Arc<EngineInner>,
    ws_order: &CcxtOrder,
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

    // 使用 Binance 推送的 rp 作为已实现盈亏，开仓时 rp=0
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

    // fill_price <= 0.0: 跳过 Trade 记录构造（防止 0.0 价格传播给回测），
    // 但仓位更新必须执行——apply_fill 对平仓单不读 fill_price，
    // 对开仓单有内部 `if fill_price > 0.0` 守卫。
    // 若此处 return 会导致仓位状态与 DB replay 不一致（project_memory lesson:
    // "fill_price <= 0.0 时直接返回不更新仓位会导致订单状态与仓位不一致"）。
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
        let trade = Trade {
            id: Uuid::new_v4(),
            position_id: position_id.unwrap_or(Uuid::nil()),
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

    // 仓位更新：原子更新 realized_pnl + quantity + entry_price + status
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
    // 注：终态清理（orders/order_position remove）由调用点统一负责，
    // 确保 Filled 状态无论 trade_fill 是否 > 0 都能清理，避免泄漏。
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

    // 如果仓位已存在，直接下单
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

    if position.quantity == 0.0 {
        let client_order_id = Uuid::new_v4().to_string();
        inner.emit_event(EngineEvent::OrderFailed {
            client_order_id,
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
    };

    // 将仓位状态改为 Closing
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

    // 解析 position_id
    let position_id = match params.position_id {
        Some(pid) => pid,
        None => {
            let position_side = match params.position_side.take() {
                Some(ps) => ps,
                None => {
                    error!(
                        client_order_id = ?params.client_order_id,
                        symbol = %params.symbol,
                        "position_side unresolved (side is Unknown), cannot place order"
                    );
                    inner.emit_event(EngineEvent::OrderFailed {
                        client_order_id: params.client_order_id.clone().unwrap_or_default(),
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
                finalize_pending_order(inner, &client_order_id, ws_order, Some(position_id)).await;
            }
            // 如果 WS 未到达，等待 WS 回调处理
        }
        Err(e) => {
            inner.pending_orders.remove(&client_order_id);
            let msg = format!("Failed to place order: {}", e);
            error!(error = %e, symbol = %symbol_for_error, "Failed to place order");

            // 回滚幽灵 Opening 仓位：place_order 失败后，若仓位仍为 Opening + size=0，说明无订单成功下达
            let pos_key = inner
                .position_id_index
                .get(&position_id)
                .map(|r| r.value().clone());
            if let Some(key) = pos_key {
                if let Some(mut pos) = inner.positions.get_mut(&key) {
                    if pos.is_ghost() {
                        let id = pos.id;
                        drop(pos);
                        inner.positions.remove(&key);
                        inner.position_id_index.remove(&id);
                        warn!(position_id = %position_id, "Removed ghost Opening position after place_order failure");
                    } else if pos.status == PositionStatus::Closing {
                        pos.rollback_to_open(Utc::now());
                        let pos_clone = pos.clone();
                        drop(pos);
                        warn!(position_id = %position_id, "Rolled back Closing position to Open after place_order failure");
                        inner.emit_event(EngineEvent::PositionUpdated {
                            position: pos_clone,
                        });
                    }
                }
            }

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
