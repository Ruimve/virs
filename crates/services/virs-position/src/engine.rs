//! PositionEngine — position management engine.
//!
//! Manages positions, orders, risk checks, PnL tracking via 4 parallel loops:
//! - command_loop: command dispatch
//! - sync_loop: position synchronization
//! - ws_feed_loop: WebSocket order update consumption
//! - poll_loop: polling fallback

use std::collections::HashMap;
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
use virs_error::{PositionEngineError, PositionResult};

use crate::persistence::PositionPersistence;
use crate::risk::{DrawdownAction, RiskChecker};
use crate::tracker::PnlTracker;

/// Recover from lock poisoning by accessing the inner data anyway.
///
/// **WARNING**: Lock poisoning means a thread panicked while holding the lock,
/// and the protected data may be in an inconsistent state. In a trading system,
/// continuing with stale/inconsistent state can lead to incorrect risk checks,
/// duplicate orders, or balance errors.
///
/// We log at `error!` level so monitoring can detect this and trigger an alert.
/// The engine should be restarted as soon as possible after lock poisoning.
///
/// This function PANICS on lock poisoning — returning dirty data is more
/// dangerous than crashing in a trading system.
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
// EngineInner
// ============================================================================

pub(crate) struct EngineInner {
    pub(crate) config: EngineConfig,
    pub(crate) exchange: Arc<dyn ExchangePe>,
    pub(crate) persistence: Box<dyn PositionPersistence>,
    pub(crate) positions: DashMap<(String, String, PositionSide), Position>,
    pub(crate) orders: DashMap<Uuid, PositionOrder>,
    pub(crate) event_tx: broadcast::Sender<EngineEvent>,
    pub(crate) risk_checker: Mutex<RiskChecker>,
    pub(crate) tracker: Mutex<PnlTracker>,
    pub(crate) state: RwLock<EngineState>,
    pub(crate) exchange_order_id_index: DashMap<String, Uuid>,
    pub(crate) position_id_index: DashMap<Uuid, (String, String, PositionSide)>,
    pub(crate) last_close_all: RwLock<Option<chrono::DateTime<Utc>>>,
}

impl EngineInner {
    fn emit_event(&self, event: EngineEvent) {
        if self.event_tx.send(event).is_err() {
            warn!("EngineEvent broadcast failed — no receivers, event dropped");
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

// ============================================================================
// PositionEngine
// ============================================================================

pub struct PositionEngine {
    inner: Arc<EngineInner>,
    cmd_tx: mpsc::Sender<EngineCommand>,
    cmd_rx: Option<mpsc::Receiver<EngineCommand>>,
}

impl Clone for PositionEngine {
    /// 手动实现 Clone：cmd_rx 不可克隆，clone 时置为 None。
    /// cmd_rx 仅在 run() 中 take() 一次使用，clone 出的实例仅用于查询，不需要 cmd_rx。
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            cmd_tx: self.cmd_tx.clone(),
            cmd_rx: None,
        }
    }
}

impl PositionEngine {
    /// 创建新的 PositionEngine 实例。
    pub fn new(
        config: EngineConfig,
        exchange: Box<dyn ExchangePe>,
        persistence: Box<dyn PositionPersistence>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let event_tx = broadcast::channel(256).0;
        let exchange: Arc<dyn ExchangePe> = Arc::from(exchange);

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
        };

        Self {
            inner: Arc::new(inner),
            cmd_tx,
            cmd_rx: Some(cmd_rx),
        }
    }

    /// 获取命令发送端。
    pub fn command_sender(&self) -> mpsc::Sender<EngineCommand> {
        self.cmd_tx.clone()
    }

    /// 订阅引擎事件。
    pub fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent> {
        self.inner.event_tx.subscribe()
    }

    /// 获取事件广播器的 sender（用于 EngineManager 暴露给 API 层订阅）
    pub fn event_sender(&self) -> broadcast::Sender<EngineEvent> {
        self.inner.event_tx.clone()
    }

    /// 获取所有仓位。
    pub fn get_all_positions(&self) -> Vec<Position> {
        self.inner
            .positions
            .iter()
            .map(|r| r.value().clone())
            .collect()
    }

    /// 按 symbol 查询当前 Open 状态的仓位（用于 bot 决策前直接查询，避免事件缓存失效）。
    pub fn get_open_position_by_symbol(&self, symbol: &str) -> Option<Position> {
        self.inner
            .positions
            .iter()
            .filter(|r| r.value().symbol == symbol && r.value().is_open())
            .map(|r| r.value().clone())
            .next()
    }

    /// 获取内部 Exchange 的共享引用。
    pub fn exchange(&self) -> Arc<dyn ExchangePe> {
        Arc::clone(&self.inner.exchange)
    }

    /// 启动引擎主循环。
    pub async fn run(&mut self) -> PositionResult<()> {
        // 1. 初始化数据库表
        self.inner.persistence.init_tables().await?;

        // VIRS is Hedge-only. Position mode is not stored or queried at runtime —
        // the frontend wizard validates Hedge mode when credentials are saved.
        // resolve_position_side_for_hedge auto-resolves position_side for callers
        // that omit it (e.g. grid bot PlaceOrder).

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
        info!(engine_id = %self.inner.config.engine_id, "Position engine started");

        // 5. 启动 4 个并行循环
        let cmd_rx = self
            .cmd_rx
            .take()
            .ok_or(PositionEngineError::ChannelClosed)?;
        let inner = Arc::clone(&self.inner);

        let mut cmd_handle = tokio::spawn(command_loop(inner.clone(), cmd_rx));
        let mut sync_handle = tokio::spawn(sync_loop(inner.clone()));
        let mut ws_handle = tokio::spawn(ws_feed_loop(inner.clone(), ws_feed_rx));
        let mut poll_handle = tokio::spawn(poll_loop(inner));

        // Wait for any task to complete (error path)
        let _ = tokio::select! {
            r = &mut cmd_handle => r,
            r = &mut sync_handle => r,
            r = &mut ws_handle => r,
            r = &mut poll_handle => r,
        };

        // Signal all loops to stop
        self.inner.set_state(EngineState::ShuttingDown);

        // ws_feed_loop exits within ~1s via select! timeout.
        // command_loop exits when all cmd_tx senders are dropped (engine_manager handles this).
        // sync_loop/poll_loop check is_running() on their next tick (default 10s interval),
        // which exceeds our 5s timeout — abort them immediately since they only do
        // read-only sync/poll operations that are safe to interrupt.
        sync_handle.abort();
        poll_handle.abort();

        // Wait for command_loop and ws_feed_loop to finish (with timeout)
        let timeout = Duration::from_secs(5);
        let _ = tokio::time::timeout(timeout, async {
            let _ = tokio::join!(cmd_handle, ws_handle);
        }).await;

        self.inner.set_state(EngineState::Stopped);
        info!(engine_id = %self.inner.config.engine_id, "Position engine stopped");
        Ok(())
    }

    /// Signal the engine to stop gracefully.
    /// Sets state to ShuttingDown, which causes sync_loop/poll_loop to break
    /// on their next tick. command_loop exits when cmd_tx senders are dropped.
    pub fn stop(&self) {
        self.inner.set_state(EngineState::ShuttingDown);
        info!(engine_id = %self.inner.config.engine_id, "Position engine stop requested");
    }

    // -----------------------------------------------------------------------
    // 状态恢复
    // -----------------------------------------------------------------------

    async fn recover_state(&self) -> PositionResult<()> {
        let engine_id = &self.inner.config.engine_id;

        let open_positions = self.inner.persistence.get_open_positions(engine_id).await?;
        for pos in &open_positions {
            let key = (pos.exchange.clone(), pos.symbol.clone(), pos.side);
            self.inner.position_id_index.insert(pos.id, key.clone());
            self.inner.positions.insert(key, pos.clone());
        }

        // 同步恢复的仓位到交易所内存状态（仅 Paper 模式需要，真实交易所空实现）
        // 避免 sync_loop 误判"本地有但交易所没有" → 强制关闭本地仓位
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
                                "Failed to persist position in full_sync"
                            );
                            self.inner.positions.insert(key, pos.clone());
                            // 发出 PositionUpdated 事件，让前端 WS 和 AutoWorker 感知仓位已恢复
                            self.inner
                                .emit_event(EngineEvent::PositionUpdated { position: pos });
                        }
                        None => {
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
                            self.inner
                                .position_id_index
                                .insert(position.id, new_key.clone());
                            persist!(
                                self.inner.persistence.upsert_position(&position),
                                "Failed to persist new position in full_sync"
                            );
                            self.inner.positions.insert(new_key, position.clone());
                            // 外部发现的仓位也发出事件
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

// ============================================================================
// command_loop
// ============================================================================

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

// ============================================================================
// sync_loop
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
                let exchange_keys: std::collections::HashSet<(String, String, PositionSide)> =
                    exchange_positions
                        .iter()
                        .map(|ep| (exchange_name.clone(), ep.symbol.clone(), ep.side))
                        .collect();

                for ep in &exchange_positions {
                    let key = (exchange_name.clone(), ep.symbol.clone(), ep.side);
                    match inner.positions.get(&key) {
                        Some(local) => {
                            let local_size = local.value().size;
                            if (local_size - ep.size).abs() > 1e-8 {
                                warn!(symbol = %ep.symbol, local_size, exchange_size = ep.size, "Position size mismatch detected");
                            }
                            let mut pos = local.value().clone();
                            // 检测关键字段是否变化（用于决定是否推送 PositionUpdated 事件）
                            let price_changed = (pos.current_price - ep.entry_price).abs() > 1e-8;
                            let pnl_changed = (pos.unrealized_pnl - ep.unrealized_pnl).abs() > 1e-8;
                            let liq_changed = pos.liquidation_price != ep.liquidation_price;
                            pos.current_price = ep.entry_price;
                            pos.unrealized_pnl = ep.unrealized_pnl;
                            pos.liquidation_price = ep.liquidation_price;
                            pos.updated_at = Utc::now();
                            drop(local);
                            persist!(
                                inner.persistence.upsert_position(&pos),
                                "Failed to persist position in sync_loop"
                            );
                            inner.positions.insert(key, pos.clone());
                            // 仓位状态变化时推送事件
                            if price_changed || pnl_changed || liq_changed {
                                inner.emit_event(EngineEvent::PositionUpdated { position: pos });
                            }
                        }
                        None => {
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
                            persist!(
                                inner.persistence.upsert_position(&position),
                                "Failed to persist new position in sync_loop"
                            );
                            inner.positions.insert(new_key, position.clone());
                            // 同步发现的新仓位也需要发出事件，让 worker 和前端感知
                            inner.emit_event(EngineEvent::PositionOpened {
                                position: position.clone(),
                            });
                            inner.emit_event(EngineEvent::PositionUpdated { position });
                        }
                    }
                }

                // 检测本地有但交易所没有的仓位
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
                        warn!(symbol = %lk.1, side = ?lk.2, "Position exists locally but not on exchange");

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
                            .map(|r| *r.key())
                            .collect();

                        for oid in &active_order_ids {
                            if let Err(e) =
                                inner.exchange.cancel_order(&lk.1, &oid.to_string()).await
                            {
                                warn!(order_id = %oid, error = %e, "Failed to cancel active order before closing position");
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
                            persist!(
                                inner.persistence.upsert_position(&closed_pos),
                                "Failed to persist closed position in sync_loop"
                            );
                            inner.emit_event(EngineEvent::PositionClosed {
                                position: closed_pos.clone(),
                            });
                            inner.emit_event(EngineEvent::PositionUpdated {
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
                        recover_lock(inner.risk_checker.lock())
                            .check_funding_rate(&sym, funding.rate)
                    };
                    if let Some(alert) = alert {
                        inner.emit_event(EngineEvent::RiskAlert {
                            level: alert.severity,
                            message: alert.message,
                        });
                    }
                }
                Err(e) => {
                    warn!(error = %e, symbol = %sym, "Failed to get funding rate — PnL calculation may be inaccurate");
                }
            }
        }

        // 3. 检查强平预警
        {
            let risk_checker = recover_lock(inner.risk_checker.lock());
            for entry in inner.positions.iter() {
                let pos = entry.value();
                // liquidation_price is critical for risk assessment — if missing,
                // skip this position's warning and log an error rather than
                // silently using 0.0 which would make the warning meaningless.
                let liq_price = match pos.liquidation_price {
                    Some(p) if p > 0.0 => p,
                    _ => {
                        error!(
                            position_id = %pos.id,
                            symbol = %pos.symbol,
                            "Position has no valid liquidation_price — \
                             cannot assess liquidation risk. Skipping warning."
                        );
                        continue;
                    }
                };
                if let Some(_distance_pct) = risk_checker.check_liquidation(pos) {
                    inner.emit_event(EngineEvent::LiquidationWarning {
                        position_id: pos.id,
                        symbol: pos.symbol.clone(),
                        liquidation_price: liq_price,
                        current_price: pos.current_price,
                    });
                    warn!(position_id = %pos.id, symbol = %pos.symbol, "Liquidation warning");
                }
            }
        }

        // 4. 更新未实现盈亏
        {
            let positions: Vec<Position> =
                inner.positions.iter().map(|r| r.value().clone()).collect();
            let position_refs: Vec<&Position> = positions.iter().collect();
            let current_prices: HashMap<String, f64> = positions
                .iter()
                .map(|p| (p.symbol.clone(), p.current_price))
                .collect();

            let snapshot = {
                recover_lock(inner.tracker.lock())
                    .update_unrealized(&position_refs, &current_prices)
            };

            // 5. 回撤检查
            let peak_equity = { recover_lock(inner.tracker.lock()).peak_equity() };
            let drawdown_action = {
                recover_lock(inner.risk_checker.lock())
                    .check_drawdown(peak_equity, snapshot.equity)
            };
            match drawdown_action {
                Some(DrawdownAction::Warning) => {
                    inner.emit_event(EngineEvent::RiskAlert {
                        level: "warning".to_string(),
                        message: format!("Drawdown warning: {:.2}%", snapshot.max_drawdown * 100.0),
                    });
                }
                Some(DrawdownAction::Pause) => {
                    inner.emit_event(EngineEvent::RiskAlert {
                        level: "warning".to_string(),
                        message: format!(
                            "Drawdown pause: {:.2}%, new positions blocked",
                            snapshot.max_drawdown * 100.0
                        ),
                    });
                }
                Some(DrawdownAction::CloseAll) => {
                    let now = Utc::now();
                    let cooldown =
                        chrono::Duration::seconds(inner.config.sync_interval_secs as i64 * 2);
                    let in_cooldown = {
                        let last = recover_lock(inner.last_close_all.read());
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
                            snapshot.max_drawdown * 100.0
                        ),
                    });

                    *recover_lock(inner.last_close_all.write()) = Some(now);

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

                    let close_futures: Vec<_> = positions_to_close.into_iter().map(|(pid, sym, side, size)| {
                        let inner = inner.clone();
                        async move {
                            let close_side = match side {
                                PositionSide::Long => Side::Sell,
                                PositionSide::Short => Side::Buy,
                            };
                            let mut params = PlaceOrderParams {
                                symbol: sym.clone(), side: close_side, order_type: OrderType::Market,
                                amount: size, price: None, reduce_only: true,
                                position_side: Some(side), position_id: Some(pid), client_order_id: None,
                            };
                            resolve_position_side_for_hedge(&mut params);
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
                                        warn!(position_id = %pid, symbol = %sym, "Emergency close order placed due to max drawdown");
                                        return;
                                    }
                                    Err(e) => {
                                        attempts += 1;
                                        if attempts >= max_attempts {
                                            error!(position_id = %pid, symbol = %sym, error = %e, attempts, "Failed to place emergency close order after retries");
                                            return;
                                        }
                                        warn!(position_id = %pid, symbol = %sym, error = %e, attempt = attempts, "Emergency close order failed, retrying");
                                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                    }
                                }
                            }
                        }
                    }).collect();
                    futures_util::future::join_all(close_futures).await;
                }
                Some(DrawdownAction::Normal) | None => {}
            }
        }
    }
}

// ============================================================================
// ws_feed_loop
// ============================================================================

pub(crate) async fn ws_feed_loop(inner: Arc<EngineInner>, mut ws_rx: OrderUpdateStream) {
    loop {
        tokio::select! {
            event = ws_rx.next() => {
                match event {
                    Some(WsFeedEvent::OrderUpdate {
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
                    }) => {
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

/// 处理 WebSocket 订单更新。
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
    // 1. 查找本地 Order
    let (order_id, position_id, prev_filled, is_reduce_only) = {
        let order_id_opt = inner
            .exchange_order_id_index
            .get(exchange_order_id)
            .map(|r| *r.value());
        let order_id = match order_id_opt {
            Some(id) => id,
            None => {
                    warn!(exchange_order_id, "Received order update for unknown order — state may be out of sync");
                    return;
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

    // Cache the position key once; it is stable for the lifetime of this call
    // (only removed at the very end when a position fully closes).
    let pos_key_opt = inner
        .position_id_index
        .get(&position_id)
        .map(|r| r.value().clone());

    // WS position_side mismatch check (uses cached key).
    if let Some(ref ws_ps) = ws_position_side {
        if let Some(ref pos_key) = pos_key_opt {
            let pos_side = &pos_key.2;
            if pos_side != ws_ps {
                warn!(exchange_order_id, order_id = %order_id, ws_position_side = ?ws_ps, local_position_side = ?pos_side, "WS position_side mismatch");
            }
        }
    }

    // 2. 更新 Order
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

    // Clone the current order state once for reuse in trade event emission
    // and position update below. The order is not modified after step 2.
    let current_order_opt = if matches!(
        status,
        OrderStatus::PartiallyFilled | OrderStatus::Filled
    ) {
        inner.orders.get(&order_id).map(|r| r.value().clone())
    } else {
        None
    };

    // 3. 部分成交或完全成交时创建 Trade 记录
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

            // Validate price — a 0.0 price indicates WS data anomaly.
            // Skip Trade construction to prevent 0.0 price propagation (same as REST path).
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
            {
                recover_lock(inner.risk_checker.lock()).record_trade_result(pnl);
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

    // 4. 订单完全成交时更新仓位
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
                    "Failed to persist position after order fill"
                );
            }
        }
    }
}

// ============================================================================
// poll_loop
// ============================================================================

pub(crate) async fn poll_loop(inner: Arc<EngineInner>) {
    let interval = tokio::time::Duration::from_secs(inner.config.poll_interval_secs);
    let mut ticker = tokio::time::interval(interval);

    loop {
        ticker.tick().await;
        if !inner.is_running() {
            break;
        }

        // 轮询交易所订单状态
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

                    if local.status != eo.status || (local.filled - eo.filled).abs() > 1e-8 {
                        warn!(order_id = %local.id, local_status = ?local.status, exchange_status = ?eo.status, "Order status mismatch detected in poll");
                        let mut updated = local.value().clone();
                        updated.status = eo.status;
                        updated.filled = eo.filled;
                        updated.remaining = eo.remaining;
                        updated.fill_price = eo.fill_price;
                        updated.fee = eo.fee;
                        updated.updated_at = Utc::now();
                        drop(local);
                        inner.orders.insert(updated.id, updated.clone());
                    }
                }
            }
            Err(_) => {}
        }
    }
}

// ============================================================================
// 命令处理函数
// ============================================================================

pub(crate) async fn handle_open_position(
    inner: &Arc<EngineInner>,
    exchange: String,
    symbol: String,
    side: PositionSide,
    _order_side: Side,
    size: f64,
    leverage: Option<u32>,
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

    // If position already exists, append order to it (e.g. grid multi-level orders)
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
                inner.orders.insert(order.id, order.clone());

                // 如果订单已成交，发出 OrderFilled 事件
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

    if let Some(lev) = leverage {
        if let Err(e) = inner.exchange.set_leverage(&symbol, lev).await {
            let msg = format!("Failed to set leverage: {}", e);
            error!(error = %e, symbol = %symbol, leverage = lev, "Failed to set leverage");
            inner.emit_event(EngineEvent::RiskAlert {
                level: "warning".to_string(),
                message: format!(
                    "Leverage inconsistency: {}x requested for {} but exchange returned error: {}",
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

    let lev = leverage.unwrap_or_else(|| {
        // No leverage specified — use the engine's configured default leverage.
        // Falling back to 1 would cause margin overestimation and incorrect
        // risk checks. Log a warning so operators know the default was used.
        warn!(
            symbol = %symbol,
            "No leverage specified for open_position — using engine default ({}x)",
            inner.config.default_leverage
        );
        inner.config.default_leverage
    });

    let total_equity = inner.exchange.get_balance().await
        .map(|b| b.total)
        .unwrap_or_else(|e| {
            warn!(error = %e, "Failed to get balance for risk check, using tracker equity as fallback");
            recover_lock(inner.tracker.lock()).equity()
        });

    {
        let positions_owned: Vec<Position> =
            inner.positions.iter().map(|r| r.value().clone()).collect();
        let positions: Vec<&Position> = positions_owned.iter().collect();
        let risk_checker = recover_lock(inner.risk_checker.lock());

        if let Err(e) =
            risk_checker.check_open_position(&positions, &symbol, size, lev, total_equity)
        {
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

    let now = Utc::now();
    let position_id = Uuid::new_v4();
    let mut position = Position {
        id: position_id,
        engine_id: inner.config.engine_id.clone(),
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
            // 确保订单关联正确的 position_id（交易所 REST 可能不返回此字段）
            order.position_id = position_id;
            position.status = PositionStatus::Open;
            position.size = order.filled;
            // fill_price is critical — a filled order without fill_price indicates
            // a data integrity issue. Using 0.0 would cause zero-cost positions,
            // infinite leverage capacity, and explosive PnL errors.
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
                        // Rollback: position was set to Open at line 1456, but
                        // we cannot confirm a valid fill_price. Revert to Opening
                        // to prevent a zero-cost Open position from entering the system.
                        position.status = PositionStatus::Opening;
                        return;
                    }
                }
            } else {
                // Order is not filled — entry_price is unknown.
                // Using 0.0 would produce a zero-cost position, which is incorrect.
                // Skip position update entirely; the order will be picked up when it fills.
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
            inner.orders.insert(order.id, order.clone());

            persist!(
                inner.persistence.upsert_position(&position),
                "Failed to persist position in open_position"
            );

            inner.emit_event(EngineEvent::PositionOpened {
                position: position.clone(),
            });
            inner.emit_event(EngineEvent::PositionUpdated {
                position: position.clone(),
            });

            // 如果订单已成交（市价单立即成交），发出 OrderFilled 事件
            // 这是 AutoWorker 等待的事件，用于确认开仓并记录交易
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

    // 加超时保护：防止交易所 REST 调用卡死导致 PE engine loop 阻塞，
    // 进而无法 emit OrderFailed 事件，bot 侧 pending_close 永远等不到响应。
    const CLOSE_ORDER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
    match tokio::time::timeout(CLOSE_ORDER_TIMEOUT, inner.exchange.place_order(params)).await {
        Ok(Ok(mut order)) => {
            order.reduce_only = reduce_only;
            // 确保订单关联正确的 position_id（交易所 REST 可能不返回此字段）
            order.position_id = position_id;
            if let Some(ref eoid) = order.exchange_order_id {
                inner.exchange_order_id_index.insert(eoid.clone(), order.id);
            }
            inner.orders.insert(order.id, order.clone());

            // 如果订单已成交，发出 OrderFilled 事件
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

                        // 市价单立即成交时，直接更新仓位状态为 Closed 并发出事件
                        // 避免等待 sync_loop 检测仓位消失（最多 10 秒延迟）
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
                                "Failed to persist closed position in close_position"
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
                CLOSE_ORDER_TIMEOUT.as_secs(),
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

/// Resolves `position_side` from the order `side` + `reduce_only` when the
/// caller omitted it. VIRS is Hedge-only — this is the only valid inference.
/// Clears `reduce_only` afterwards: Binance Hedge mode uses positionSide
/// (not reduceOnly) for position management, so reduceOnly is unnecessary.
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

    let total_equity = inner
        .exchange
        .get_balance()
        .await
        .map(|b| b.total)
        .unwrap_or_else(|e| {
            warn!(error = %e, "Failed to get balance for risk check");
            recover_lock(inner.tracker.lock()).equity()
        });
    {
        let positions_owned: Vec<Position> =
            inner.positions.iter().map(|r| r.value().clone()).collect();
        let positions: Vec<&Position> = positions_owned.iter().collect();
        let risk_checker = recover_lock(inner.risk_checker.lock());
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

    // Auto-create position if not provided (e.g. from Grid bot PlaceOrder)
    let position_id = match params.position_id {
        Some(pid) => pid,
        None => {
            let pos_id = Uuid::new_v4();
            // resolve_position_side_for_hedge has already resolved position_side
            // from side+reduce_only. If it's still None here, that's a bug.
            let position_side = params.position_side
                .expect("position_side must be resolved by resolve_position_side_for_hedge");
            let exchange_name = inner.exchange.name().to_string();
            let key = (exchange_name.clone(), params.symbol.clone(), position_side);

            // Reuse existing position if one already exists for this key
            if let Some(existing) = inner.positions.get(&key) {
                existing.id
            } else {
                let position = Position {
                    id: pos_id,
                    engine_id: inner.config.engine_id.clone(),
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
                    "Failed to persist auto-created position"
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
