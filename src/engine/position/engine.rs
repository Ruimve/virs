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
use super::persistence::{PositionPersistence, PnlSnapshotRow};
use super::risk::{DrawdownAction, RiskChecker};
use super::tracker::PnlTracker;
use super::types::*;

macro_rules! persist {
    ($expr:expr, $label:expr) => {
        let mut attempts = 0u32;
        loop {
            match $expr.await {
                Ok(()) => break,
                Err(e) => {
                    attempts += 1;
                    if attempts >= 3 {
                        error!(error = %e, attempts, $label);
                        break;
                    }
                    warn!(error = %e, attempt = attempts, $label);
                    tokio::time::sleep(std::time::Duration::from_millis(100 * attempts as u64)).await;
                }
            }
        }
    };
}

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
    pub(crate) persistence: Box<dyn PositionPersistence>,
    pub(crate) positions: DashMap<(String, String, PositionSide), Position>,
    pub(crate) orders: DashMap<Uuid, Order>,
    pub(crate) event_tx: broadcast::Sender<EngineEvent>,
    pub(crate) risk_checker: Mutex<RiskChecker>,
    pub(crate) tracker: Mutex<PnlTracker>,
    pub(crate) state: RwLock<EngineState>,
    pub(crate) exchange_order_id_index: DashMap<String, Uuid>,
    pub(crate) position_id_index: DashMap<Uuid, (String, String, PositionSide)>,
    pub(crate) last_close_all: RwLock<Option<chrono::DateTime<Utc>>>,
    pub(crate) position_mode: RwLock<PositionMode>,
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
    pub fn new(config: EngineConfig, exchange: Box<dyn Exchange>, persistence: Box<dyn PositionPersistence>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let event_tx = broadcast::channel(256).0;

        let inner = EngineInner {
            persistence,
            risk_checker: Mutex::new(RiskChecker::new(config.risk.clone())),
            tracker: Mutex::new(PnlTracker::new(0.0)),
            state: RwLock::new(EngineState::Created),
            config,
            exchange,
            event_tx,
            positions: DashMap::new(),
            orders: DashMap::new(),
            exchange_order_id_index: DashMap::new(),
            position_id_index: DashMap::new(),
            last_close_all: RwLock::new(None),
            position_mode: RwLock::new(PositionMode::OneWay),
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

        // 1.5 获取持仓模式（双向/单向）
        match self.inner.exchange.get_position_mode().await {
            Ok(mode) => {
                info!(
                    engine_id = %self.inner.config.engine_id,
                    position_mode = ?mode,
                    "Fetched position mode from exchange"
                );
                *self.inner.position_mode.write().unwrap() = mode;
            }
            Err(e) => {
                warn!(
                    engine_id = %self.inner.config.engine_id,
                    error = %e,
                    "Failed to get position mode from exchange"
                );
                return Err(PositionEngineError::PositionModeQueryFailed(e.to_string()));
            }
        }

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
            self.inner.position_id_index.insert(pos.id, key.clone());
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
            if let Some(ref eoid) = order.exchange_order_id {
                self.inner.exchange_order_id_index.insert(eoid.clone(), order.id);
            }
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
            tracker.restore_from_snapshot(
                snapshot.peak_equity,
                snapshot.total_realized_pnl,
                snapshot.total_trades as u32,
                snapshot.profit_trades as u32,
                snapshot.total_cost,
                snapshot.consecutive_losses as u32,
            );
            info!(
                engine_id = %engine_id,
                realized_pnl = snapshot.total_realized_pnl,
                peak_equity = snapshot.peak_equity,
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
                            persist!(self.inner.persistence.upsert_position(&pos), "Failed to persist position in poll_loop");
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
                            self.inner.position_id_index.insert(position.id, new_key.clone());
                            persist!(self.inner.persistence.upsert_position(&position), "Failed to persist new position in poll_loop");
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
                persist!(inner.persistence.insert_event(
                    &inner.config.engine_id,
                    "engine_shutting_down",
                    None,
                    "Engine shutting down",
                    "info",
                ), "Failed to persist shutdown event");
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
                let exchange_keys: std::collections::HashSet<(String, String, PositionSide)> = exchange_positions
                    .iter()
                    .map(|ep| (exchange_name.clone(), ep.symbol.clone(), ep.side))
                    .collect();

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
                            // 仅更新价格和盈亏字段，不覆盖 size
                            // size 由 WS 回调实时维护，避免旧快照覆盖新数据
                            let mut pos = local.value().clone();
                            pos.current_price = ep.entry_price;
                            pos.unrealized_pnl = ep.unrealized_pnl;
                            pos.liquidation_price = ep.liquidation_price;
                            pos.updated_at = Utc::now();
                            drop(local);
                            persist!(inner.persistence.upsert_position(&pos), "Failed to persist position in sync_loop");
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
                            inner.position_id_index.insert(position.id, new_key.clone());
                            persist!(inner.persistence.upsert_position(&position), "Failed to persist new position in sync_loop");
                            inner.positions.insert(new_key, position);
                        }
                    }
                }

                // 检测本地有但交易所没有的仓位（可能被外部强平）
                let local_keys: Vec<(String, String, PositionSide)> = inner
                    .positions
                    .iter()
                    .filter_map(|r| {
                        let pos = r.value();
                        if pos.exchange == exchange_name && pos.status != PositionStatus::Closed {
                            Some((pos.exchange.clone(), pos.symbol.clone(), pos.side))
                        } else {
                            None
                        }
                    })
                    .collect();
                for lk in &local_keys {
                    if !exchange_keys.contains(lk) {
                        warn!(
                            symbol = %lk.1,
                            side = ?lk.2,
                            "Position exists locally but not on exchange (possibly liquidated externally)"
                        );

                        let active_order_ids: Vec<Uuid> = inner
                            .orders
                            .iter()
                            .filter(|r| {
                                let o = r.value();
                                o.symbol == lk.1
                                    && !matches!(
                                        o.status,
                                        OrderStatus::Filled
                                            | OrderStatus::Canceled
                                            | OrderStatus::Failed
                                    )
                            })
                            .map(|r| r.key().clone())
                            .collect();

                        for oid in &active_order_ids {
                            if let Err(e) = inner.exchange.cancel_order(&lk.1, &oid.to_string()).await {
                                warn!(order_id = %oid, error = %e, "Failed to cancel active order before closing position in sync_loop");
                            }
                            if let Some(mut order) = inner.orders.get_mut(oid) {
                                order.status = OrderStatus::Canceled;
                            }
                        }

                        if let Some(mut pos) = inner.positions.get_mut(lk) {
                            pos.status = PositionStatus::Closed;
                            pos.size = 0.0;
                            pos.closed_at = Some(Utc::now());
                            pos.updated_at = Utc::now();
                            let closed_pos = pos.clone();
                            drop(pos);
                            inner.position_id_index.remove(&closed_pos.id);
                            inner.positions.remove(lk);
                            persist!(inner.persistence.upsert_position(&closed_pos), "Failed to persist closed position in sync_loop");
                            inner.emit_event(EngineEvent::PositionClosed {
                                position: closed_pos,
                            });
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
        let funding_futures: Vec<_> = symbols_to_check
            .iter()
            .map(|sym| {
                let sym = sym.clone();
                let inner = inner.clone();
                async move {
                    let result = inner.exchange.get_funding_rate(&sym).await;
                    (sym, result)
                }
            })
            .collect();
        let funding_results = futures_util::future::join_all(funding_futures).await;
        for (sym, result) in funding_results {
            match result {
                Ok(funding) => {
                    let alert = {
                        let risk_checker = inner.risk_checker.lock().unwrap();
                        risk_checker.check_funding_rate(&sym, funding.rate)
                    };
                    if let Some(alert) = alert {
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
        {
            let risk_checker = inner.risk_checker.lock().unwrap();
            for entry in inner.positions.iter() {
                let pos = entry.value();
                if let Some(_distance_pct) = risk_checker.check_liquidation(pos) {
                    inner.emit_event(EngineEvent::LiquidationWarning {
                        position_id: pos.id,
                        symbol: pos.symbol.clone(),
                        liquidation_price: pos.liquidation_price.unwrap_or(0.0),
                        current_price: pos.current_price,
                    });
                    warn!(
                        position_id = %pos.id,
                        symbol = %pos.symbol,
                        "Liquidation warning"
                    );
                }
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
                peak_equity: {
                    let tracker = inner.tracker.lock().unwrap();
                    tracker.peak_equity()
                },
                total_trades: {
                    let tracker = inner.tracker.lock().unwrap();
                    tracker.total_trades() as i32
                },
                profit_trades: {
                    let tracker = inner.tracker.lock().unwrap();
                    tracker.profit_trades() as i32
                },
                total_cost: {
                    let tracker = inner.tracker.lock().unwrap();
                    tracker.total_cost()
                },
                consecutive_losses: {
                    let tracker = inner.tracker.lock().unwrap();
                    tracker.consecutive_losses() as i32
                },
            };
            persist!(inner.persistence.insert_pnl_snapshot(&inner.config.engine_id, &pnl_row), "Failed to persist pnl snapshot");
        }

        // 5. 检查回撤
        let drawdown_check = {
            let tracker = inner.tracker.lock().unwrap();
            let peak = tracker.peak_equity();
            let snap = tracker.snapshot(0.0, inner.positions.len());
            let risk_checker = inner.risk_checker.lock().unwrap();
            let action = risk_checker.check_drawdown(peak, snap.equity);
            (action, snap)
        };
        if let Some(action) = drawdown_check.0 {
            let snap = &drawdown_check.1;
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
                    let now = Utc::now();
                    let cooldown = chrono::Duration::seconds(inner.config.sync_interval_secs as i64 * 2);
                    let in_cooldown = {
                        let last = inner.last_close_all.read().unwrap();
                        last.map(|t| now - t < cooldown).unwrap_or(false)
                    };
                    if in_cooldown {
                        warn!("CloseAll in cooldown, skipping duplicate trigger");
                        continue;
                    }

                    inner.emit_event(EngineEvent::RiskAlert {
                        level: "critical".to_string(),
                        message: format!(
                            "Max drawdown exceeded, closing all positions: {:.2}%",
                            snap.max_drawdown * 100.0
                        ),
                    });

                    *inner.last_close_all.write().unwrap() = Some(now);

                    let positions_to_close: Vec<(Uuid, String, PositionSide, f64)> = inner
                        .positions
                        .iter()
                        .filter_map(|r| {
                            let pos = r.value();
                            if pos.size > 0.0 && pos.status != PositionStatus::Closing {
                                Some((pos.id, pos.symbol.clone(), pos.side, pos.size))
                            } else {
                                None
                            }
                        })
                        .collect();

                    for (pid, _, _, _) in &positions_to_close {
                        let pos_key = inner.position_id_index.get(pid).map(|r| r.value().clone());
                        if let Some(key) = pos_key {
                            if let Some(mut pos) = inner.positions.get_mut(&key) {
                                pos.status = PositionStatus::Closing;
                                pos.updated_at = now;
                            }
                        }
                    }
                    let close_futures: Vec<_> = positions_to_close
                        .into_iter()
                        .map(|(pid, sym, side, size)| {
                            let inner = inner.clone();
                            async move {
                                let close_side = match side {
                                    PositionSide::Long => Side::Sell,
                                    PositionSide::Short => Side::Buy,
                                    PositionSide::Both => Side::Sell,
                                };
                                let mut params = PlaceOrderParams {
                                    symbol: sym.clone(),
                                    side: close_side,
                                    order_type: OrderType::Market,
                                    amount: size,
                                    price: None,
                                    reduce_only: true,
                                    position_side: Some(side),
                                    position_id: Some(pid),
                                    client_order_id: None,
                                };
                                let mode = *inner.position_mode.read().unwrap();
                                adjust_params_for_position_mode(&mut params, mode);
                                let mut attempts = 0u32;
                                let max_attempts = 3;
                                loop {
                                    match inner.exchange.place_order(params.clone()).await {
                                        Ok(mut order) => {
                                            order.reduce_only = params.reduce_only;
                                            if let Some(ref eoid) = order.exchange_order_id {
                                                inner.exchange_order_id_index.insert(eoid.clone(), order.id);
                                            }
                                            inner.orders.insert(order.id, order.clone());
                                            persist!(inner.persistence.insert_order(&order), "Failed to persist emergency close order");
                                            warn!(
                                                position_id = %pid,
                                                symbol = %sym,
                                                "Emergency close order placed due to max drawdown"
                                            );
                                            return;
                                        }
                                        Err(e) => {
                                            attempts += 1;
                                            if attempts >= max_attempts {
                                                error!(
                                                    position_id = %pid,
                                                    symbol = %sym,
                                                    error = %e,
                                                    attempts,
                                                    "Failed to place emergency close order after retries"
                                                );
                                                return;
                                            }
                                            warn!(
                                                position_id = %pid,
                                                symbol = %sym,
                                                error = %e,
                                                attempt = attempts,
                                                "Emergency close order failed, retrying"
                                            );
                                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                        }
                                    }
                                }
                            }
                        })
                        .collect();
                    futures_util::future::join_all(close_futures).await;
                }
                DrawdownAction::Normal => {}
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
                position_side,
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
                    position_side,
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
                persist!(inner.persistence.insert_event(
                    &inner.config.engine_id,
                    event_type,
                    None,
                    message,
                    severity,
                ), "Failed to persist ws connection event");
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
    ws_position_side: Option<PositionSide>,
) {
    // 1. 查找本地 Order（通过 exchange_order_id_index O(1) 查找）
    let (order_id, position_id, prev_filled, is_reduce_only) = {
        let order_id_opt = inner.exchange_order_id_index.get(exchange_order_id).map(|r| *r.value());
        let order_id = match order_id_opt {
            Some(id) => id,
            None => {
                debug!(
                    exchange_order_id,
                    "Received order update for unknown order"
                );
                return;
            }
        };
        let order = match inner.orders.get(&order_id) {
            Some(o) => o,
            None => {
                debug!(
                    exchange_order_id,
                    "Order index points to missing order"
                );
                return;
            }
        };

        if let Some(ref ws_ps) = ws_position_side {
            if let Some(pos_key) = inner.position_id_index.get(&order.position_id).map(|r| r.value().clone()) {
                let pos_side = &pos_key.2;
                let expected_side = match ws_ps {
                    PositionSide::Long => PositionSide::Long,
                    PositionSide::Short => PositionSide::Short,
                    PositionSide::Both => PositionSide::Both,
                };
                if pos_side != &expected_side {
                    warn!(
                        exchange_order_id,
                        order_id = %order.id,
                        ws_position_side = ?ws_ps,
                        local_position_side = ?pos_side,
                        "WS position_side mismatch with local position side"
                    );
                }
            }
        }

        let is_reduce_only = {
            let order_reduce = order.reduce_only;
            if order_reduce {
                true
            } else if let Some(ref ws_ps) = ws_position_side {
                match (&order.side, ws_ps) {
                    (Side::Sell, PositionSide::Long) => true,
                    (Side::Buy, PositionSide::Short) => true,
                    _ => false,
                }
            } else {
                false
            }
        };

        (order.id, order.position_id, order.filled, is_reduce_only)
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
            persist!(inner.persistence.update_order(&updated_order), "Failed to persist order update in ws_feed");
        }
    }

    // 3. 部分成交或完全成交时创建 Trade 记录
    if matches!(status, OrderStatus::PartiallyFilled | OrderStatus::Filled) {
        let trade_fill = filled - prev_filled;

        if trade_fill < 0.0 {
            warn!(
                order_id = %order_id,
                prev_filled,
                new_filled = filled,
                "WS order update out of order: filled decreased, skipping PnL calculation"
            );
            return;
        }

        if trade_fill > 0.0 {
            // 计算 PnL
            let (pnl, trade_side) = {
                let pos_key = inner.position_id_index.get(&position_id).map(|r| r.value().clone());
                match pos_key {
                    Some(key) => {
                        let pos_entry = inner.positions.get(&key);
                        match pos_entry {
                            Some(pe) => {
                                let pos = pe.value();
                                if is_reduce_only {
                                    let p = match pos.side {
                                        PositionSide::Long => (price - pos.entry_price) * trade_fill,
                                        PositionSide::Short => (pos.entry_price - price) * trade_fill,
                                        PositionSide::Both => {
                                            if pos.size >= 0.0 {
                                                (price - pos.entry_price) * trade_fill
                                            } else {
                                                (pos.entry_price - price) * trade_fill
                                            }
                                        }
                                    };
                                    let side = match pos.side {
                                        PositionSide::Long => Side::Sell,
                                        PositionSide::Short => Side::Buy,
                                        PositionSide::Both => {
                                            if pos.size >= 0.0 { Side::Sell } else { Side::Buy }
                                        }
                                    };
                                    (p, side)
                                } else {
                                    let side = match pos.side {
                                        PositionSide::Long => Side::Buy,
                                        PositionSide::Short => Side::Sell,
                                        PositionSide::Both => {
                                            if pos.size >= 0.0 { Side::Buy } else { Side::Sell }
                                        }
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
                trade_type: if is_reduce_only { TradeType::Close } else { TradeType::Open },
                created_at: timestamp,
            };

            persist!(inner.persistence.insert_trade(&trade), "Failed to persist trade");

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

            // 更新仓位已实现盈亏（平仓成交时累加）
            if pnl != 0.0 {
                let pos_key = inner.position_id_index.get(&position_id).map(|r| r.value().clone());
                if let Some(key) = pos_key {
                    if let Some(mut pos) = inner.positions.get_mut(&key) {
                        pos.realized_pnl += pnl;
                    }
                }
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
        let pos_key = inner.position_id_index.get(&position_id).map(|r| r.value().clone());
        let pos_entry = match pos_key {
            Some(key) => inner.positions.get(&key).map(|r| r.value().clone()),
            None => None,
        };

        if let Some(mut position) = pos_entry {
            let order = inner.orders.get(&order_id).map(|r| r.value().clone());

            if let Some(order) = order {
                if is_reduce_only {
                    // 平仓订单成交（realized_pnl 已在 trade 处理阶段累加）
                    position.size -= order.filled;

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
                    inner.position_id_index.remove(&position.id);
                    inner.positions.remove(&key);
                } else {
                    inner.positions.insert(key, position.clone());
                }
                persist!(inner.persistence.upsert_position(&position), "Failed to persist position in ws_feed close");
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
                    let eoid = match eo.exchange_order_id.as_deref() {
                        Some(id) => id,
                        None => continue,
                    };
                    let local_id = match inner.exchange_order_id_index.get(eoid) {
                        Some(r) => *r.value(),
                        None => continue,
                    };
                    let local = match inner.orders.get(&local_id) {
                        Some(r) => r,
                        None => continue,
                    };

                    if local.status != eo.status
                        || (local.filled - eo.filled).abs() > 1e-8
                    {
                        warn!(
                            order_id = %local.id,
                            local_status = ?local.status,
                            exchange_status = ?eo.status,
                            "Order status mismatch detected in poll"
                        );
                        let mut updated = local.value().clone();
                        updated.status = eo.status;
                        updated.filled = eo.filled;
                        updated.remaining = eo.remaining;
                        updated.fill_price = eo.fill_price;
                        updated.fee = eo.fee;
                        updated.updated_at = Utc::now();
                        drop(local);
                        inner.orders.insert(updated.id, updated.clone());
                        persist!(inner.persistence.update_order(&updated), "Failed to persist order update in ws_feed partial");
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
    order_side: Side,
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
            error!(error = %e, symbol = %symbol, leverage = lev, "Failed to set leverage");
            inner.emit_event(EngineEvent::RiskAlert {
                level: "warning".to_string(),
                message: format!(
                    "Leverage inconsistency: {}x requested for {} but exchange returned error: {}. Position may have incorrect leverage.",
                    lev, symbol, e
                ),
            });
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
        .unwrap_or_else(|e| {
            warn!(error = %e, "Failed to get balance for risk check, using tracker equity as fallback");
            inner.tracker.lock().unwrap().equity()
        });

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
    let resolved_side = match side {
        PositionSide::Long => Side::Buy,
        PositionSide::Short => Side::Sell,
        PositionSide::Both => order_side,
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
        client_order_id: None,
    };
    let mode = *inner.position_mode.read().unwrap();
    adjust_params_for_position_mode(&mut params, mode);
    let reduce_only = params.reduce_only;

    match inner.exchange.place_order(params).await {
        Ok(mut order) => {
            order.reduce_only = reduce_only;
            position.status = PositionStatus::Open;
            position.size = order.filled;
            position.entry_price = order.fill_price.unwrap_or(0.0);
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
            inner.orders.insert(order.id, order.clone());

            persist!(inner.persistence.upsert_position(&position), "Failed to persist position in open_position");
            persist!(inner.persistence.insert_order(&order), "Failed to persist order in open_position");

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
    let position = {
        let pos_key = inner.position_id_index.get(&position_id).map(|r| r.value().clone());
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
        PositionSide::Both => {
            if position.size >= 0.0 {
                Side::Sell
            } else {
                Side::Buy
            }
        }
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
        client_order_id: None,
    };
    let mode = *inner.position_mode.read().unwrap();
    adjust_params_for_position_mode(&mut params, mode);
    let reduce_only = params.reduce_only;

    match inner.exchange.place_order(params).await {
        Ok(mut order) => {
            order.reduce_only = reduce_only;
            if let Some(ref eoid) = order.exchange_order_id {
                inner.exchange_order_id_index.insert(eoid.clone(), order.id);
            }
            inner.orders.insert(order.id, order.clone());
            persist!(inner.persistence.insert_order(&order), "Failed to persist order in close_position");
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
        .position_id_index
        .get(&position_id)
        .map(|r| r.value().clone());

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
        persist!(inner.persistence.upsert_position(&updated), "Failed to persist position in modify_position");
        inner.emit_event(EngineEvent::PositionModified {
            position_id,
            stop_loss,
            take_profit,
        });
        info!(position_id = %position_id, "Position modified");
    }
}

/// 根据持仓模式调整下单参数。
///
/// - Hedge 模式：position_side 必须为 Long/Short（不能为 None 或 Both），reduce_only 不传给交易所
/// - OneWay 模式：position_side 不传给交易所，平仓时传 reduce_only
pub(crate) fn adjust_params_for_position_mode(params: &mut PlaceOrderParams, mode: PositionMode) {
    match mode {
        PositionMode::Hedge => {
            if params.position_side.is_none() || params.position_side == Some(PositionSide::Both) {
                params.position_side = match (&params.side, params.reduce_only) {
                    (Side::Buy, false) => Some(PositionSide::Long),
                    (Side::Sell, false) => Some(PositionSide::Short),
                    (Side::Sell, true) => Some(PositionSide::Long),
                    (Side::Buy, true) => Some(PositionSide::Short),
                };
            }
            params.reduce_only = false;
        }
        PositionMode::OneWay => {
            params.position_side = None;
        }
    }
}

/// 处理通用下单命令。
pub(crate) async fn handle_place_order(inner: &Arc<EngineInner>, mut params: PlaceOrderParams) {
    let mode = *inner.position_mode.read().unwrap();
    adjust_params_for_position_mode(&mut params, mode);

    // 风控检查
    let total_equity = inner.exchange.get_balance().await
        .map(|b| b.total)
        .unwrap_or_else(|e| {
            warn!(error = %e, "Failed to get balance for risk check, using tracker equity as fallback");
            inner.tracker.lock().unwrap().equity()
        });
    {
        let positions_owned: Vec<Position> = inner.positions.iter().map(|r| r.value().clone()).collect();
        let positions: Vec<&Position> = positions_owned.iter().collect();
        let risk_checker = inner.risk_checker.lock().unwrap();
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
        Ok(mut order) => {
            if let Some(pid) = params.position_id {
                order.position_id = pid;
            }
            order.reduce_only = params.reduce_only;
            inner.orders.insert(order.id, order.clone());
            persist!(inner.persistence.insert_order(&order), "Failed to persist order in place_order");
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
            persist!(inner.persistence.update_order(&cancelled_order), "Failed to persist cancelled order in cancel_order");
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

    match inner.exchange.cancel_all_orders(target_symbol.as_deref()).await {
        Ok(cancelled_orders) => {
            for order in &cancelled_orders {
                inner.orders.insert(order.id, order.clone());
                persist!(inner.persistence.update_order(order), "Failed to persist cancelled order");
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
    use crate::engine::position::persistence::Persistence;
    use crate::engine::position::config::RiskConfig;

    pub(crate) fn make_test_inner(
        config: EngineConfig,
        exchange: Box<dyn Exchange>,
    ) -> Arc<EngineInner> {
        let db = PgPool::connect_lazy("postgres://__test__:__test__@localhost/__test__")
            .expect("connect_lazy should not fail");
        let event_tx = broadcast::channel(256).0;

        Arc::new(EngineInner {
            persistence: Box::new(Persistence::new(db)),
            risk_checker: Mutex::new(RiskChecker::new(config.risk.clone())),
            tracker: Mutex::new(PnlTracker::new(10000.0)),
            state: RwLock::new(EngineState::Running),
            config,
            exchange,
            event_tx,
            positions: DashMap::new(),
            orders: DashMap::new(),
            exchange_order_id_index: DashMap::new(),
            position_id_index: DashMap::new(),
            last_close_all: RwLock::new(None),
            position_mode: RwLock::new(PositionMode::Hedge),
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
