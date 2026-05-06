use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use sqlx::PgPool;
use tokio::sync::{broadcast, mpsc};
use dashmap::DashMap;
use tracing::{debug, error, info, warn};
use chrono::Utc;
use uuid::Uuid;

use super::config::EngineConfig;
use super::error::*;
use super::exchange::Exchange;
use super::persistence::{Persistence, PnlSnapshotRow};
use super::risk::{DrawdownAction, RiskChecker};
use super::tracker::PnlTracker;
use super::types::*;

// ============================================================================
// EngineInner - 内部共享状态
// ============================================================================

/// 引擎内部共享状态，通过 `Arc` 在多个异步任务间共享。
///
/// - `positions` / `orders`: DashMap 本身线程安全，无需额外锁。
/// - `risk_checker` / `tracker`: 内部有可变状态，用 `std::sync::Mutex` 保护。
/// - `state`: 多读少写，用 `std::sync::RwLock` 保护。
pub(crate) struct EngineInner {
    pub(crate) config: EngineConfig,
    pub(crate) exchange: Box<dyn Exchange>,
    pub(crate) persistence: Persistence,
    pub(crate) positions: DashMap<(String, String, PositionSide), Position>,
    pub(crate) orders: DashMap<Uuid, Order>,
    pub(crate) event_tx: broadcast::Sender<EngineEvent>,
    pub(crate) risk_checker: Mutex<RiskChecker>,
    pub(crate) tracker: Mutex<PnlTracker>,
    pub(crate) state: RwLock<EngineState>,
}

impl EngineInner {
    /// 发出引擎事件（忽略接收端已关闭的情况）。
    fn emit_event(&self, event: EngineEvent) {
        let _ = self.event_tx.send(event);
    }

    /// 检查引擎是否处于 Running 状态。
    fn is_running(&self) -> bool {
        *self.state.read().unwrap() == EngineState::Running
    }

    /// 设置引擎状态。
    fn set_state(&self, new_state: EngineState) {
        *self.state.write().unwrap() = new_state;
    }

    /// 读取引擎状态。
    fn get_state(&self) -> EngineState {
        *self.state.read().unwrap()
    }
}

// ============================================================================
// PositionEngine - 公开 API
// ============================================================================

pub struct PositionEngine {
    inner: Arc<EngineInner>,
    cmd_tx: mpsc::Sender<EngineCommand>,
    cmd_rx: Option<mpsc::Receiver<EngineCommand>>,
}

impl PositionEngine {
    /// 创建新的 PositionEngine 实例。
    ///
    /// 调用 [`run()`](Self::run) 启动引擎主循环。
    pub fn new(config: EngineConfig, exchange: Box<dyn Exchange>, db: PgPool) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let event_tx = broadcast::channel(256).0;

        let inner = EngineInner {
            persistence: Persistence::new(db),
            risk_checker: Mutex::new(RiskChecker::new(config.risk.clone())),
            tracker: Mutex::new(PnlTracker::new(0.0)),
            state: RwLock::new(EngineState::Created),
            config,
            exchange,
            event_tx,
            positions: DashMap::new(),
            orders: DashMap::new(),
        };

        Self {
            inner: Arc::new(inner),
            cmd_tx,
            cmd_rx: Some(cmd_rx),
        }
    }

    /// 获取命令发送端，用于向引擎发送命令。
    pub fn command_sender(&self) -> mpsc::Sender<EngineCommand> {
        self.cmd_tx.clone()
    }

    /// 订阅引擎事件。
    pub fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent> {
        self.inner.event_tx.subscribe()
    }

    /// 获取指定仓位。
    pub fn get_position(
        &self,
        exchange: &str,
        symbol: &str,
        side: PositionSide,
    ) -> Option<Position> {
        let key = (exchange.to_string(), symbol.to_string(), side);
        self.inner.positions.get(&key).map(|r| r.value().clone())
    }

    /// 获取所有仓位。
    pub fn get_all_positions(&self) -> Vec<Position> {
        self.inner
            .positions
            .iter()
            .map(|r| r.value().clone())
            .collect()
    }

    /// 获取引擎当前状态。
    pub fn state(&self) -> EngineState {
        self.inner.get_state()
    }

    /// 启动引擎主循环，阻塞直到引擎停止。
    ///
    /// 内部启动 4 个并行异步任务：
    /// - `command_loop` - 命令分发
    /// - `sync_loop` - 仓位同步
    /// - `ws_feed_loop` - WebSocket 消费
    /// - `poll_loop` - 轮询兜底
    pub async fn run(&mut self) -> Result<()> {
        // 1. 初始化数据库表
        self.inner.persistence.init_tables().await?;

        // 2. 从数据库恢复状态
        self.recover_state().await?;

        // 3. 订阅 WebSocket 成交回报
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

        // 4. 设置状态为 Running
        self.inner.set_state(EngineState::Running);
        self.inner
            .persistence
            .insert_event(
                &self.inner.config.engine_id,
                "engine_started",
                None,
                "Engine started",
                "info",
            )
            .await?;
        info!(
            engine_id = %self.inner.config.engine_id,
            "Position engine started"
        );

        // 5. 启动 4 个并行循环
        let cmd_rx = self.cmd_rx.take().ok_or(PositionEngineError::ChannelClosed)?;
        let inner = Arc::clone(&self.inner);

        let cmd_handle = tokio::spawn(command_loop(inner.clone(), cmd_rx));
        let sync_handle = tokio::spawn(sync_loop(inner.clone()));
        let ws_handle = tokio::spawn(ws_feed_loop(inner.clone(), ws_feed_rx));
        let poll_handle = tokio::spawn(poll_loop(inner));

        // 等待任意循环结束（通常是 command_loop 收到 Shutdown 后退出）
        // 其他循环检测到非 Running 状态后会自行退出
        let _ = tokio::select! {
            r = cmd_handle => r,
            r = sync_handle => r,
            r = ws_handle => r,
            r = poll_handle => r,
        };

        self.inner.set_state(EngineState::Stopped);
        info!(
            engine_id = %self.inner.config.engine_id,
            "Position engine stopped"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // 状态恢复
    // -----------------------------------------------------------------------

    /// 从数据库恢复引擎状态：加载未关闭的仓位、活跃订单，并执行一次全量同步。
    async fn recover_state(&self) -> Result<()> {
        let engine_id = &self.inner.config.engine_id;

        // 从数据库加载未关闭的 Position
        let open_positions = self
            .inner
            .persistence
            .get_open_positions(engine_id)
            .await?;
        for pos in open_positions {
            let key = (pos.exchange.clone(), pos.symbol.clone(), pos.side);
            self.inner.positions.insert(key, pos);
        }
        info!(
            engine_id = %engine_id,
            count = self.inner.positions.len(),
            "Recovered positions from database"
        );

        // 从数据库加载活跃的 Order
        let active_orders = self
            .inner
            .persistence
            .get_active_orders(engine_id)
            .await?;
        for order in active_orders {
            self.inner.orders.insert(order.id, order);
        }
        info!(
            engine_id = %engine_id,
            count = self.inner.orders.len(),
            "Recovered orders from database"
        );

        // 恢复 PnlTracker（从最新快照近似恢复）
        if let Some(snapshot) = self
            .inner
            .persistence
            .get_latest_snapshot(engine_id)
            .await?
        {
            let mut tracker = self.inner.tracker.lock().unwrap();
            let approx_peak = (snapshot.total_realized_pnl + snapshot.total_unrealized_pnl)
                .max(snapshot.total_realized_pnl);
            tracker.restore_from_snapshot(approx_peak, snapshot.total_realized_pnl, 0, 0, 0.0);
            info!(
                engine_id = %engine_id,
                realized_pnl = snapshot.total_realized_pnl,
                "Restored PnlTracker from snapshot"
            );
        }

        // 执行一次全量同步
        self.full_sync().await;

        Ok(())
    }

    /// 执行一次全量仓位同步。
    async fn full_sync(&self) {
        let exchange_name = self.inner.exchange.name().to_string();

        match self.inner.exchange.get_positions(None).await {
            Ok(exchange_positions) => {
                for ep in &exchange_positions {
                    let key = (exchange_name.clone(), ep.symbol.clone(), ep.side);
                    match self.inner.positions.get(&key) {
                        Some(local) => {
                            // 更新价格和未实现盈亏
                            let mut pos = local.value().clone();
                            pos.current_price = ep.entry_price;
                            pos.unrealized_pnl = ep.unrealized_pnl;
                            pos.liquidation_price = ep.liquidation_price;
                            pos.updated_at = Utc::now();
                            drop(local);
                            self.inner.persistence.upsert_position(&pos).await.ok();
                            self.inner.positions.insert(key, pos);
                        }
                        None => {
                            // 外部开仓 —— 创建本地 Position
                            let now = Utc::now();
                            let position = Position {
                                id: Uuid::new_v4(),
                                engine_id: self.inner.config.engine_id.clone(),
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
                            self.inner.persistence.upsert_position(&position).await.ok();
                            self.inner.positions.insert(new_key, position);
                            info!(symbol = %ep.symbol, "Recovered external position");
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

// ============================================================================
// command_loop - 命令分发循环
// ============================================================================

pub(crate) async fn command_loop(inner: Arc<EngineInner>, mut cmd_rx: mpsc::Receiver<EngineCommand>) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            EngineCommand::OpenPosition {
                exchange,
                symbol,
                side,
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
            } => {
                handle_close_position(&inner, position_id, order_type, price).await;
            }
            EngineCommand::ModifyPosition {
                position_id,
                stop_loss,
                take_profit,
            } => {
                handle_modify_position(&inner, position_id, stop_loss, take_profit).await;
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
            EngineCommand::SyncPositions => {
                handle_sync_positions(&inner).await;
            }
            EngineCommand::Shutdown => {
                info!("Shutdown command received");
                inner.set_state(EngineState::ShuttingDown);
                inner
                    .persistence
                    .insert_event(
                        &inner.config.engine_id,
                        "engine_shutting_down",
                        None,
                        "Engine shutting down",
                        "info",
                    )
                    .await
                    .ok();
                break;
            }
        }
    }
    debug!("Command loop exited");
}

// ============================================================================
// sync_loop - 仓位同步循环
// ============================================================================

pub(crate) async fn sync_loop(inner: Arc<EngineInner>) {
    let interval = tokio::time::Duration::from_secs(inner.config.sync_interval_secs);
    let mut ticker = tokio::time::interval(interval);

    loop {
        ticker.tick().await;
        if !inner.is_running() {
            break;
        }

        // 1. 从交易所获取仓位，与本地比对
        let exchange_name = inner.exchange.name().to_string();
        match inner.exchange.get_positions(None).await {
            Ok(exchange_positions) => {
                for ep in &exchange_positions {
                    let key = (exchange_name.clone(), ep.symbol.clone(), ep.side);
                    match inner.positions.get(&key) {
                        Some(local) => {
                            let local_size = local.value().size;
                            if (local_size - ep.size).abs() > 1e-8 {
                                warn!(
                                    symbol = %ep.symbol,
                                    local_size,
                                    exchange_size = ep.size,
                                    "Position size mismatch detected"
                                );
                            }
                            // 无论是否 mismatch，都更新价格和盈亏
                            let mut pos = local.value().clone();
                            pos.size = ep.size;
                            pos.current_price = ep.entry_price;
                            pos.unrealized_pnl = ep.unrealized_pnl;
                            pos.liquidation_price = ep.liquidation_price;
                            pos.updated_at = Utc::now();
                            drop(local);
                            inner.persistence.upsert_position(&pos).await.ok();
                            inner.positions.insert(key, pos);
                        }
                        None => {
                            // 外部开仓
                            info!(symbol = %ep.symbol, "New position detected from exchange");
                            let now = Utc::now();
                            let position = Position {
                                id: Uuid::new_v4(),
                                engine_id: inner.config.engine_id.clone(),
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
                            inner.persistence.upsert_position(&position).await.ok();
                            inner.positions.insert(new_key, position);
                        }
                    }
                }

                inner.emit_event(EngineEvent::PositionSynced {
                    positions: exchange_positions,
                });
            }
            Err(e) => {
                error!(error = %e, "Failed to sync positions");
            }
        }

        // 2. 检查资金费率
        let symbols_to_check: Vec<String> = inner
            .positions
            .iter()
            .map(|r| r.value().symbol.clone())
            .collect();
        for sym in &symbols_to_check {
            match inner.exchange.get_funding_rate(sym).await {
                Ok(funding) => {
                    let risk_checker = inner.risk_checker.lock().unwrap();
                    if let Some(alert) = risk_checker.check_funding_rate(sym, funding.rate) {
                        inner.emit_event(EngineEvent::RiskAlert {
                            level: alert.severity,
                            message: alert.message,
                        });
                    }
                }
                Err(e) => {
                    debug!(error = %e, symbol = %sym, "Failed to get funding rate");
                }
            }
        }

        // 3. 检查强平预警
        for entry in inner.positions.iter() {
            let pos = entry.value();
            let risk_checker = inner.risk_checker.lock().unwrap();
            if let Some(distance_pct) = risk_checker.check_liquidation(pos) {
                inner.emit_event(EngineEvent::LiquidationWarning {
                    position_id: pos.id,
                    symbol: pos.symbol.clone(),
                    liquidation_price: pos.liquidation_price.unwrap_or(0.0),
                    current_price: pos.current_price,
                });
                warn!(
                    position_id = %pos.id,
                    symbol = %pos.symbol,
                    distance_pct,
                    "Liquidation warning"
                );
            }
        }

        // 4. 更新未实现盈亏并写入 PnL 快照
        {
            let positions: Vec<Position> = inner
                .positions
                .iter()
                .map(|r| r.value().clone())
                .collect();
            let position_refs: Vec<&Position> = positions.iter().collect();
            let current_prices: HashMap<String, f64> = positions
                .iter()
                .map(|p| (p.symbol.clone(), p.current_price))
                .collect();

            let snapshot = {
                let mut tracker = inner.tracker.lock().unwrap();
                tracker.update_unrealized(&position_refs, &current_prices)
            };

            let pnl_row = PnlSnapshotRow {
                id: Uuid::new_v4(),
                engine_id: inner.config.engine_id.clone(),
                timestamp: snapshot.timestamp,
                total_unrealized_pnl: snapshot.unrealized_pnl,
                total_realized_pnl: snapshot.realized_pnl,
                total_pnl: snapshot.total_pnl,
                position_count: positions.len() as i32,
                open_position_count: snapshot.open_positions_count as i32,
                total_margin: positions.iter().map(|p| p.margin).sum(),
                drawdown_pct: snapshot.max_drawdown,
            };
            inner
                .persistence
                .insert_pnl_snapshot(&inner.config.engine_id, &pnl_row)
                .await
                .ok();
        }

        // 5. 检查回撤
        {
            let tracker = inner.tracker.lock().unwrap();
            let peak = tracker.peak_equity();
            let snap = tracker.snapshot(0.0, inner.positions.len());
            drop(tracker);

            let risk_checker = inner.risk_checker.lock().unwrap();
            if let Some(action) = risk_checker.check_drawdown(peak, snap.equity) {
                match action {
                    DrawdownAction::Warning => {
                        inner.emit_event(EngineEvent::RiskAlert {
                            level: "warning".to_string(),
                            message: format!(
                                "Drawdown warning: {:.2}%",
                                snap.max_drawdown * 100.0
                            ),
                        });
                    }
                    DrawdownAction::Pause => {
                        inner.emit_event(EngineEvent::RiskAlert {
                            level: "critical".to_string(),
                            message: format!(
                                "Drawdown critical, pausing new positions: {:.2}%",
                                snap.max_drawdown * 100.0
                            ),
                        });
                    }
                    DrawdownAction::CloseAll => {
                        inner.emit_event(EngineEvent::RiskAlert {
                            level: "critical".to_string(),
                            message: format!(
                                "Max drawdown exceeded, closing all positions: {:.2}%",
                                snap.max_drawdown * 100.0
                            ),
                        });
                        // TODO: 触发平仓所有仓位
                    }
                    DrawdownAction::Normal => {}
                }
            }
        }
    }
    debug!("Sync loop exited");
}

// ============================================================================
// ws_feed_loop - WebSocket 消费循环
// ============================================================================

pub(crate) async fn ws_feed_loop(inner: Arc<EngineInner>, mut ws_rx: mpsc::Receiver<WsFeedEvent>) {
    while let Some(event) = ws_rx.recv().await {
        match event {
            WsFeedEvent::OrderUpdate {
                exchange_order_id,
                symbol,
                status,
                filled,
                remaining,
                price,
                amount,
                commission,
                timestamp,
            } => {
                handle_ws_order_update(
                    &inner,
                    &exchange_order_id,
                    &symbol,
                    status,
                    filled,
                    remaining,
                    price,
                    amount,
                    commission,
                    timestamp,
                )
                .await;
            }
            WsFeedEvent::ConnectionChanged { connected } => {
                info!(connected, "WebSocket connection changed");
                let (event_type, severity, message) = if connected {
                    ("ws_reconnected", "info", "WebSocket reconnected")
                } else {
                    ("ws_disconnected", "warning", "WebSocket disconnected")
                };
                inner
                    .persistence
                    .insert_event(
                        &inner.config.engine_id,
                        event_type,
                        None,
                        message,
                        severity,
                    )
                    .await
                    .ok();
            }
        }
    }
    debug!("WebSocket feed loop exited");
}

/// 处理 WebSocket 订单更新。
///
/// 流程：查找本地 Order -> 更新 Order 状态 -> 写入 Trade -> 更新 Position -> 发出事件。
pub(crate) async fn handle_ws_order_update(
    inner: &Arc<EngineInner>,
    exchange_order_id: &str,
    symbol: &str,
    status: OrderStatus,
    filled: f64,
    remaining: f64,
    price: f64,
    amount: f64,
    commission: f64,
    timestamp: chrono::DateTime<Utc>,
) {
    // 1. 查找本地 Order（通过 exchange_order_id 匹配）
    let (order_id, position_id, prev_filled) = {
        let entry = inner.orders.iter().find(|r| {
            r.value()
                .exchange_order_id
                .as_deref()
                .map(|id| id == exchange_order_id)
                .unwrap_or(false)
        });
        match entry {
            Some(e) => {
                let o = e.value();
                (o.id, o.position_id, o.filled)
            }
            None => {
                debug!(
                    exchange_order_id,
                    "Received order update for unknown order"
                );
                return;
            }
        }
    };

    // 2. 更新 Order
    {
        if let Some(mut order) = inner.orders.get_mut(&order_id) {
            order.filled = filled;
            order.remaining = remaining;
            order.fill_price = Some(price);
            order.fee = commission;
            order.status = status;
            order.updated_at = timestamp;
            let updated_order = order.clone();
            drop(order);
            inner.persistence.update_order(&updated_order).await.ok();
        }
    }

    // 3. 部分成交或完全成交时创建 Trade 记录
    if matches!(status, OrderStatus::PartiallyFilled | OrderStatus::Filled) {
        let trade_fill = (filled - prev_filled).max(0.0);

        if trade_fill > 0.0 {
            // 计算 PnL
            let (pnl, trade_side) = {
                let pos_entry = inner
                    .positions
                    .iter()
                    .find(|r| r.value().id == position_id);
                match pos_entry {
                    Some(pe) => {
                        let pos = pe.value();
                        let p = match pos.side {
                            PositionSide::Long => (price - pos.entry_price) * trade_fill,
                            PositionSide::Short => (pos.entry_price - price) * trade_fill,
                            PositionSide::Both => 0.0,
                        };
                        (p, pos.side)
                    }
                    None => (0.0, PositionSide::Both),
                }
            };

            let trade = Trade {
                id: Uuid::new_v4(),
                position_id,
                order_id,
                exchange: inner.exchange.name().to_string(),
                symbol: symbol.to_string(),
                side: match trade_side {
                    PositionSide::Long => Side::Buy,
                    PositionSide::Short => Side::Sell,
                    PositionSide::Both => Side::Buy,
                },
                price,
                amount: trade_fill,
                fee: commission,
                fee_currency: String::new(),
                pnl,
                trade_type: "trade".to_string(),
                created_at: timestamp,
            };

            inner.persistence.insert_trade(&trade).await.ok();

            // 更新 PnlTracker
            {
                let mut tracker = inner.tracker.lock().unwrap();
                tracker.record_trade(&trade);
            }

            // 更新 RiskChecker 连续亏损计数
            {
                let mut risk_checker = inner.risk_checker.lock().unwrap();
                risk_checker.record_trade_result(pnl);
            }

            // 发出事件
            let current_order = inner
                .orders
                .get(&order_id)
                .map(|r| r.value().clone())
                .unwrap_or_else(|| Order {
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

    // 4. 订单完全成交时更新仓位
    if status == OrderStatus::Filled {
        let pos_entry = inner
            .positions
            .iter()
            .find(|r| r.value().id == position_id)
            .map(|r| r.value().clone());

        if let Some(mut position) = pos_entry {
            let order = inner.orders.get(&order_id).map(|r| r.value().clone());

            if let Some(order) = order {
                if order.reduce_only {
                    // 平仓订单成交
                    position.size -= order.filled;
                    position.realized_pnl += if let Some(fp) = order.fill_price {
                        match position.side {
                            PositionSide::Long => (fp - position.entry_price) * order.filled,
                            PositionSide::Short => (position.entry_price - fp) * order.filled,
                            PositionSide::Both => 0.0,
                        }
                    } else {
                        0.0
                    };

                    if position.size.abs() < 1e-8 {
                        position.size = 0.0;
                        position.status = PositionStatus::Closed;
                        position.closed_at = Some(timestamp);
                        inner.emit_event(EngineEvent::PositionClosed {
                            position: position.clone(),
                        });
                    } else {
                        position.status = PositionStatus::Open;
                    }
                } else {
                    // 开仓订单成交
                    let old_size = position.size;
                    position.size += order.filled;
                    if let Some(fp) = order.fill_price {
                        if old_size > 0.0 && position.entry_price > 0.0 {
                            // 加权平均入场价
                            let total_cost =
                                position.entry_price * old_size + fp * order.filled;
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
                    inner.positions.remove(&key);
                } else {
                    inner.positions.insert(key, position.clone());
                }
                inner.persistence.upsert_position(&position).await.ok();
            }
        }
    }
}

// ============================================================================
// poll_loop - 轮询兜底循环
// ============================================================================

pub(crate) async fn poll_loop(inner: Arc<EngineInner>) {
    let interval = tokio::time::Duration::from_secs(inner.config.poll_interval_secs);
    let mut ticker = tokio::time::interval(interval);

    loop {
        ticker.tick().await;
        if !inner.is_running() {
            break;
        }

        // 获取交易所所有活跃订单，与本地比对
        match inner.exchange.get_open_orders(None).await {
            Ok(exchange_orders) => {
                for eo in &exchange_orders {
                    // 通过 exchange_order_id 匹配本地订单
                    let matched = inner.orders.iter().find(|r| {
                        r.value()
                            .exchange_order_id
                            .as_deref()
                            .map(|id| id == eo.exchange_order_id.as_deref().unwrap_or(""))
                            .unwrap_or(false)
                    });

                    if let Some(entry) = matched {
                        let local = entry.value();
                        if local.status != eo.status
                            || (local.filled - eo.filled).abs() > 1e-8
                        {
                            warn!(
                                order_id = %local.id,
                                local_status = ?local.status,
                                exchange_status = ?eo.status,
                                "Order status mismatch detected in poll"
                            );
                            let mut updated = local.clone();
                            updated.status = eo.status;
                            updated.filled = eo.filled;
                            updated.remaining = eo.remaining;
                            updated.fill_price = eo.fill_price;
                            updated.fee = eo.fee;
                            updated.updated_at = Utc::now();
                            drop(entry);
                            inner.orders.insert(updated.id, updated.clone());
                            inner.persistence.update_order(&updated).await.ok();
                        }
                    }
                }
            }
            Err(e) => {
                debug!(error = %e, "Failed to poll open orders");
            }
        }
    }
    debug!("Poll loop exited");
}

// ============================================================================
// 命令处理函数
// ============================================================================

/// 处理开仓命令。
pub(crate) async fn handle_open_position(
    inner: &Arc<EngineInner>,
    exchange: String,
    symbol: String,
    side: PositionSide,
    size: f64,
    leverage: Option<u32>,
    order_type: OrderType,
    price: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    strategy_id: Option<String>,
) {
    let key = (exchange.clone(), symbol.clone(), side);

    // 检查是否已有仓位
    if inner.positions.contains_key(&key) {
        let msg = format!("Position already exists: {}/{}", exchange, symbol);
        warn!(msg);
        inner.emit_event(EngineEvent::OrderFailed {
            order_id: Uuid::nil(),
            reason: msg,
        });
        return;
    }

    // 设置杠杆
    if let Some(lev) = leverage {
        if let Err(e) = inner.exchange.set_leverage(&symbol, lev).await {
            let msg = format!("Failed to set leverage: {}", e);
            error!(error = %e, "Failed to set leverage for {}", symbol);
            inner.emit_event(EngineEvent::OrderFailed {
                order_id: Uuid::nil(),
                reason: msg,
            });
            return;
        }
    }

    // 风控检查
    let lev = leverage.unwrap_or(1);

    // 获取余额来计算 total_equity（在锁之前调用 async）
    let total_equity = inner.exchange.get_balance().await
        .map(|b| b.total)
        .unwrap_or(0.0);

    {
        let positions_owned: Vec<Position> = inner.positions.iter().map(|r| r.value().clone()).collect();
        let positions: Vec<&Position> = positions_owned.iter().collect();
        let risk_checker = inner.risk_checker.lock().unwrap();

        if let Err(e) = risk_checker.check_open_position(&positions, &symbol, size, lev, total_equity) {
            let msg = format!("Risk check failed: {}", e);
            warn!(error = %e, "Risk check failed for {}", symbol);
            inner.emit_event(EngineEvent::RiskAlert {
                level: "warning".to_string(),
                message: msg.clone(),
            });
            inner.emit_event(EngineEvent::OrderFailed {
                order_id: Uuid::nil(),
                reason: msg,
            });
            return;
        }
    }

    // 创建 Position（状态 Opening）
    let now = Utc::now();
    let position_id = Uuid::new_v4();
    let mut position = Position {
        id: position_id,
        engine_id: inner.config.engine_id.clone(),
        strategy_id: strategy_id.clone(),
        exchange: exchange.clone(),
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

    // 构造下单参数
    let order_side = match side {
        PositionSide::Long => Side::Buy,
        PositionSide::Short => Side::Sell,
        PositionSide::Both => Side::Buy,
    };

    let params = PlaceOrderParams {
        symbol: symbol.clone(),
        side: order_side,
        order_type,
        amount: size,
        price,
        reduce_only: false,
        position_side: Some(side),
    };

    match inner.exchange.place_order(params).await {
        Ok(order) => {
            position.status = PositionStatus::Open;
            position.size = order.filled;
            position.entry_price = order.fill_price.unwrap_or(0.0);
            position.current_price = position.entry_price;
            position.margin = if lev > 0 {
                position.size * position.entry_price / lev as f64
            } else {
                0.0
            };

            inner.positions.insert(key, position.clone());
            inner.orders.insert(order.id, order.clone());

            inner.persistence.upsert_position(&position).await.ok();
            inner.persistence.insert_order(&order).await.ok();

            inner.emit_event(EngineEvent::PositionOpened {
                position: position.clone(),
            });
            inner.emit_event(EngineEvent::OrderPlaced {
                order: order.clone(),
            });

            info!(
                position_id = %position.id,
                symbol = %symbol,
                side = ?side,
                size = order.filled,
                "Position opened"
            );
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

/// 处理平仓命令。
pub(crate) async fn handle_close_position(
    inner: &Arc<EngineInner>,
    position_id: Uuid,
    order_type: OrderType,
    price: Option<f64>,
) {
    // 查找仓位
    let position = inner
        .positions
        .iter()
        .find(|r| r.value().id == position_id)
        .map(|r| r.value().clone());

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

    if position.size <= 0.0 {
        let msg = format!("Position {} has zero size", position_id);
        inner.emit_event(EngineEvent::OrderFailed {
            order_id: Uuid::nil(),
            reason: msg,
        });
        return;
    }

    let close_side = match position.side {
        PositionSide::Long => Side::Sell,
        PositionSide::Short => Side::Buy,
        PositionSide::Both => Side::Sell,
    };

    let params = PlaceOrderParams {
        symbol: position.symbol.clone(),
        side: close_side,
        order_type,
        amount: position.size,
        price,
        reduce_only: true,
        position_side: Some(position.side),
    };

    match inner.exchange.place_order(params).await {
        Ok(order) => {
            inner.orders.insert(order.id, order.clone());
            inner.persistence.insert_order(&order).await.ok();
            inner.emit_event(EngineEvent::OrderPlaced { order });

            // 标记仓位为 Closing
            let key = (
                position.exchange.clone(),
                position.symbol.clone(),
                position.side,
            );
            if let Some(mut pos) = inner.positions.get_mut(&key) {
                pos.status = PositionStatus::Closing;
                pos.updated_at = Utc::now();
            }

            info!(
                position_id = %position_id,
                symbol = %position.symbol,
                "Close order placed"
            );
        }
        Err(e) => {
            let msg = format!("Failed to place close order: {}", e);
            error!(error = %e, position_id = %position_id, "Failed to place close order");
            inner.emit_event(EngineEvent::OrderFailed {
                order_id: Uuid::nil(),
                reason: msg,
            });
        }
    }
}

/// 处理修改仓位命令（更新止损 / 止盈）。
pub(crate) async fn handle_modify_position(
    inner: &Arc<EngineInner>,
    position_id: Uuid,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
) {
    // 查找仓位的 DashMap key
    let key_opt = inner
        .positions
        .iter()
        .find(|r| r.value().id == position_id)
        .map(|r| r.key().clone());

    let key = match key_opt {
        Some(k) => k,
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

    if let Some(mut pos) = inner.positions.get_mut(&key) {
        pos.stop_loss = stop_loss;
        pos.take_profit = take_profit;
        pos.updated_at = Utc::now();
        let updated = pos.clone();
        drop(pos);
        inner.persistence.upsert_position(&updated).await.ok();
        inner.emit_event(EngineEvent::PositionModified {
            position_id,
            stop_loss,
            take_profit,
        });
        info!(position_id = %position_id, "Position modified");
    }
}

/// 处理通用下单命令。
pub(crate) async fn handle_place_order(inner: &Arc<EngineInner>, params: PlaceOrderParams) {
    // 风控检查
    {
        let positions_owned: Vec<Position> = inner.positions.iter().map(|r| r.value().clone()).collect();
        let positions: Vec<&Position> = positions_owned.iter().collect();
        let risk_checker = inner.risk_checker.lock().unwrap();
        let total_equity = inner.tracker.lock().unwrap().equity();
        if let Err(e) =
            risk_checker.check_place_order(&positions, &params.symbol, params.amount, total_equity)
        {
            let msg = format!("Risk check failed: {}", e);
            warn!(error = %e, symbol = %params.symbol, "Risk check failed for place order");
            inner.emit_event(EngineEvent::OrderFailed {
                order_id: Uuid::nil(),
                reason: msg,
            });
            return;
        }
    }

    match inner.exchange.place_order(params.clone()).await {
        Ok(order) => {
            inner.orders.insert(order.id, order.clone());
            inner.persistence.insert_order(&order).await.ok();
            inner.emit_event(EngineEvent::OrderPlaced {
                order: order.clone(),
            });
            info!(
                order_id = %order.id,
                symbol = %params.symbol,
                "Order placed"
            );
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

/// 处理取消订单命令。
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
            let msg = format!("Order {} has no exchange_order_id", order_id);
            inner.emit_event(EngineEvent::OrderFailed {
                order_id,
                reason: msg,
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
            inner
                .persistence
                .update_order(&cancelled_order)
                .await
                .ok();
            inner.emit_event(EngineEvent::OrderCanceled {
                order: cancelled_order,
            });
            info!(order_id = %order_id, "Order canceled");
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

/// 处理取消所有订单命令。
pub(crate) async fn handle_cancel_all_orders(
    inner: &Arc<EngineInner>,
    position_id: Option<Uuid>,
    symbol: Option<String>,
) {
    // 确定要取消的 symbol
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

    // cancel_all_orders 需要 &str，如果没有指定 symbol 则使用空字符串表示全部取消
    let symbol_arg = target_symbol.as_deref().unwrap_or("");
    match inner.exchange.cancel_all_orders(symbol_arg).await {
        Ok(cancelled_orders) => {
            for order in &cancelled_orders {
                inner.orders.insert(order.id, order.clone());
                inner.persistence.update_order(order).await.ok();
                inner.emit_event(EngineEvent::OrderCanceled {
                    order: order.clone(),
                });
            }
            info!(count = cancelled_orders.len(), "Orders canceled");
        }
        Err(e) => {
            error!(error = %e, "Failed to cancel all orders");
        }
    }
}

/// 处理手动同步命令。
pub(crate) async fn handle_sync_positions(inner: &Arc<EngineInner>) {
    match inner.exchange.get_positions(None).await {
        Ok(exchange_positions) => {
            inner.emit_event(EngineEvent::PositionSynced {
                positions: exchange_positions,
            });
            info!("Manual sync completed");
        }
        Err(e) => {
            error!(error = %e, "Failed to sync positions");
        }
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use crate::engine::position::config::RiskConfig;

    pub(crate) fn make_test_inner(
        config: EngineConfig,
        exchange: Box<dyn Exchange>,
    ) -> Arc<EngineInner> {
        let db = PgPool::connect_lazy("postgres://__test__:__test__@localhost/__test__")
            .expect("connect_lazy should not fail");
        let event_tx = broadcast::channel(256).0;

        Arc::new(EngineInner {
            persistence: Persistence::new(db),
            risk_checker: Mutex::new(RiskChecker::new(config.risk.clone())),
            tracker: Mutex::new(PnlTracker::new(10000.0)),
            state: RwLock::new(EngineState::Running),
            config,
            exchange,
            event_tx,
            positions: DashMap::new(),
            orders: DashMap::new(),
        })
    }

    pub(crate) fn collect_events(rx: &mut broadcast::Receiver<EngineEvent>, max: usize) -> Vec<EngineEvent> {
        let mut events = Vec::new();
        while events.len() < max {
            match rx.try_recv() {
                Ok(e) => events.push(e),
                Err(_) => break,
            }
        }
        events
    }
}
