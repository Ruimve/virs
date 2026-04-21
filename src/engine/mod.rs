pub mod backtest;
pub mod indicators;
pub mod lua_executor;
pub mod plugin;
pub mod plugins;
pub mod position;

use crate::models::*;
use crate::exchange::Exchange;
use crate::engine::plugin::PluginRegistry;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use uuid::Uuid;

/// Strategy execution engine.
/// Manages running strategies, generates signals, and dispatches orders.
pub struct StrategyEngine {
    strategies: Arc<DashMap<Uuid, RunningStrategy>>,
    exchange_instances: Arc<DashMap<String, Box<dyn Exchange>>>,
    order_tx: mpsc::Sender<OrderCommand>,
    config: StrategyEngineConfig,
    plugins: Arc<PluginRegistry>,
    ws_broadcaster: Option<Arc<crate::api::ws::WsBroadcaster>>,
}

#[derive(Debug, Clone)]
pub struct StrategyEngineConfig {
    pub executor_workers: usize,
    pub pending_order_poll_interval_secs: u64,
    pub auto_restore: bool,
}

#[derive(Debug)]
struct RunningStrategy {
    strategy: Strategy,
    cancel_token: tokio::sync::watch::Sender<bool>,
}

#[derive(Debug)]
pub enum OrderCommand {
    Place {
        strategy_id: Uuid,
        symbol: String,
        signal_type: SignalType,
        side: Side,
        amount: f64,
        price: Option<f64>,
        order_type: OrderType,
        exchange_name: String,
    },
    Cancel {
        strategy_id: Uuid,
        symbol: String,
        order_id: String,
        exchange_name: String,
    },
    Shutdown,
}

impl StrategyEngine {
    pub fn new(
        config: StrategyEngineConfig,
        order_tx: mpsc::Sender<OrderCommand>,
        plugins: Arc<PluginRegistry>,
    ) -> Self {
        Self {
            strategies: Arc::new(DashMap::new()),
            exchange_instances: Arc::new(DashMap::new()),
            order_tx,
            config,
            plugins,
            ws_broadcaster: None,
        }
    }

    /// Set the WebSocket broadcaster for real-time event push.
    pub fn set_ws_broadcaster(&mut self, broadcaster: Arc<crate::api::ws::WsBroadcaster>) {
        self.ws_broadcaster = Some(broadcaster);
    }

    /// Emit a WebSocket event to all connected clients.
    fn emit_event(&self, event: crate::api::ws::WsEvent) {
        if let Some(ref broadcaster) = self.ws_broadcaster {
            let _ = broadcaster.send(event);
        }
    }

    /// Register an exchange instance.
    pub fn register_exchange(&self, exchange: Box<dyn Exchange>) {
        let name = exchange.name().to_string();
        info!("Registered exchange: {}", name);
        self.exchange_instances.insert(name, exchange);
    }

    /// Register an exchange instance for a specific user.
    /// Uses a user-scoped key "{exchange}:{user_id}" so different users can
    /// have different credentials for the same exchange.
    pub fn register_exchange_for_user(
        &self,
        exchange: Box<dyn Exchange>,
        user_id: Uuid,
    ) -> String {
        let raw_name = exchange.name().to_string();
        let scoped_name = format!("{}:{}", raw_name, user_id);
        info!("Registered exchange '{}' for user {}", raw_name, user_id);
        self.exchange_instances.insert(scoped_name.clone(), exchange);
        scoped_name
    }

    /// Get a reference to an exchange instance by name.
    pub fn get_exchange(&self, name: &str) -> Option<dashmap::mapref::one::Ref<String, Box<dyn Exchange>>> {
        self.exchange_instances.get(name)
    }

    /// Get a list of registered exchange names.
    pub fn registered_exchange_names(&self) -> Vec<String> {
        self.exchange_instances.iter().map(|r| r.key().clone()).collect()
    }

    /// Start a strategy execution.
    /// If `exchange_key` is provided, use that specific exchange instance
    /// (e.g., a user-scoped one like "binance:{user_id}").
    /// Otherwise, fall back to the strategy's `exchange` field.
    pub async fn start_strategy(&self, strategy: Strategy, exchange_key: Option<String>) -> anyhow::Result<()> {
        let strategy_id = strategy.id;
        let exchange_name = exchange_key.unwrap_or_else(|| strategy.exchange.clone());
        let symbol = strategy.symbol.clone();
        let timeframe = strategy.timeframe.clone();
        let interval_secs = strategy.decide_interval_secs;
        let trading_config = strategy.trading_config.clone();
        let strategies = self.strategies.clone();
        let exchanges = self.exchange_instances.clone();
        let order_tx = self.order_tx.clone();
        let plugins = self.plugins.clone();

        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);

        strategies.insert(
            strategy_id,
            RunningStrategy {
                strategy: strategy.clone(),
                cancel_token: cancel_tx,
            },
        );

        info!(
            "Starting strategy {} ({} on {} / {})",
            strategy.name, symbol, exchange_name, timeframe
        );

        // Emit WebSocket event: strategy started
        self.emit_event(crate::api::ws::WsEvent::StrategyStatus {
            strategy_id: strategy_id.to_string(),
            name: strategy.name.clone(),
            status: "running".to_string(),
        });

        // Spawn strategy task
        let ws_broadcaster = self.ws_broadcaster.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                interval_secs as u64,
            ));

            // Initialize position manager for this strategy
            let mut pos_manager = position::PositionManager::new(
                strategy_id,
                symbol.clone(),
                exchange_name.clone(),
                &trading_config,
            );

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Some(exchange) = exchanges.get(&exchange_name) {
                            // Extract current position info for LuaContext
                            let position_info = pos_manager.position_info().map(|p| {
                                (
                                    match p.side {
                                        PositionSide::Long => "long".to_string(),
                                        PositionSide::Short => "short".to_string(),
                                    },
                                    p.entry_price,
                                    p.size,
                                )
                            });

                            match run_strategy_cycle(
                                &*exchange,
                                &symbol,
                                &timeframe,
                                &strategy,
                                &plugins,
                                position_info,
                            ).await {
                                Ok((signal, klines)) => {
                                    // Check risk management on existing position before processing new signal
                                    if let Some((close_side, close_amount, risk_reason)) = pos_manager.check_risk(klines.last().map(|k| k.close).unwrap_or(0.0)) {
                                        let risk_price = klines.last().map(|k| k.close).unwrap_or(0.0);
                                        info!(
                                            "Strategy {} risk management triggered close: {:?} for {}",
                                            strategy.name, close_side, symbol
                                        );

                                        if let Some(ref bc) = ws_broadcaster {
                                            let _ = bc.send(crate::api::ws::WsEvent::Risk {
                                                strategy_id: strategy_id.to_string(),
                                                symbol: symbol.clone(),
                                                reason: risk_reason.as_str().to_string(),
                                                price: risk_price,
                                            });
                                        }

                                        let _ = order_tx.send(OrderCommand::Place {
                                            strategy_id,
                                            symbol: symbol.clone(),
                                            signal_type: if close_side == Side::Sell { SignalType::CloseLong } else { SignalType::CloseShort },
                                            side: close_side,
                                            amount: close_amount,
                                            price: None,
                                            order_type: OrderType::Market,
                                            exchange_name: exchange_name.clone(),
                                        }).await;
                                        continue;
                                    }

                                    match signal {
                                        Some(signal) => {
                                            info!(
                                                "Strategy {} generated signal: {:?} for {}",
                                                strategy.name, signal, symbol
                                            );

                                            let amount = pos_manager.determine_amount_async(&*exchange).await;
                                            if amount <= 0.0 {
                                                warn!(
                                                    "Strategy {} signal {:?} produced amount=0, skipping",
                                                    strategy.name, signal
                                                );
                                                continue;
                                            }

                                            let (side, order_amount) = pos_manager.calculate_order(&signal);
                                            if order_amount <= 0.0 {
                                                warn!(
                                                    "Strategy {} signal {:?} rejected by position manager",
                                                    strategy.name, signal
                                                );
                                                continue;
                                            }

                                            if matches!(signal, SignalType::OpenLong | SignalType::OpenShort) {
                                                if let Some(k) = klines.last() {
                                                    pos_manager.set_entry_price(k.close);
                                                }
                                            }

                                            let _ = order_tx.send(OrderCommand::Place {
                                                strategy_id,
                                                symbol: symbol.clone(),
                                                signal_type: signal.clone(),
                                                side: side.clone(),
                                                amount: order_amount,
                                                price: None,
                                                order_type: OrderType::Market,
                                                exchange_name: exchange_name.clone(),
                                            }).await;

                                            if let Some(ref bc) = ws_broadcaster {
                                                let ref_price = klines.last().map(|k| k.close).unwrap_or(0.0);
                                                let side_str = match side {
                                                    Side::Buy => "buy",
                                                    Side::Sell => "sell",
                                                };
                                                let _ = bc.send(crate::api::ws::WsEvent::Trade {
                                                    strategy_id: strategy_id.to_string(),
                                                    symbol: symbol.clone(),
                                                    side: side_str.to_string(),
                                                    price: ref_price,
                                                    amount: order_amount,
                                                    pnl: 0.0,
                                                });
                                            }
                                        }
                                        None => {
                                            tracing::debug!("Strategy {} no signal", strategy.name);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Strategy {} error: {}", strategy.name, e);
                                }
                            }
                        } else {
                            warn!("Exchange {} not found for strategy {}", exchange_name, strategy.name);
                        }
                    }
                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            info!("Strategy {} stopped", strategy.name);
                            break;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop a running strategy.
    pub fn stop_strategy(&self, strategy_id: &Uuid) -> bool {
        if let Some(entry) = self.strategies.get_mut(strategy_id) {
            let name = entry.strategy.name.clone();
            let _ = entry.cancel_token.send(true);
            info!("Stopping strategy {}", strategy_id);

            // Emit WebSocket event: strategy stopped
            self.emit_event(crate::api::ws::WsEvent::StrategyStatus {
                strategy_id: strategy_id.to_string(),
                name,
                status: "stopped".to_string(),
            });

            true
        } else {
            false
        }
    }

    /// Get all running strategy IDs.
    pub fn running_strategy_ids(&self) -> Vec<Uuid> {
        self.strategies.iter().map(|r| *r.key()).collect()
    }

    /// Check if a strategy is running.
    pub fn is_running(&self, strategy_id: &Uuid) -> bool {
        self.strategies.contains_key(strategy_id)
    }
}

/// Run a single strategy decision cycle.
///
/// The strategy's `strategy_code` field contains user-defined script code.
/// If `strategy_mode` is "script", the code is interpreted to generate signals.
/// If `strategy_mode` is "signal", the `indicator_config` JSON is used with
/// a registered indicator plugin.
///
/// Signal mapping depends on the strategy's `trade_direction` trading config:
/// - `trade_direction = "long"`: signal 1 -> OpenLong, -1 -> CloseLong
/// - `trade_direction = "short"`: signal 1 -> CloseShort, -1 -> OpenShort
/// - `trade_direction = "both"`: signal 1 -> OpenLong, -1 -> OpenShort (close signals handled by position manager)
///
/// This enables long-only, short-only, and long/short strategies from the same indicator logic.
async fn run_strategy_cycle(
    exchange: &dyn Exchange,
    symbol: &str,
    timeframe: &str,
    strategy: &Strategy,
    plugins: &PluginRegistry,
    position_info: Option<(String, f64, f64)>,  // (side, entry_price, size)
) -> anyhow::Result<(Option<SignalType>, Vec<Kline>)> {
    let klines = exchange.get_klines(symbol, timeframe, 200, None).await?;

    if klines.len() < 50 {
        return Ok((None, klines));
    }

    let idx = klines.len() - 1;

    // Check trade direction for this strategy
    let trade_direction = strategy
        .trading_config
        .get("trade_direction")
        .and_then(|v| v.as_str())
        .unwrap_or("long");

    // Generate signal based on strategy mode
    let raw_signal: i8 = match strategy.strategy_mode {
        StrategyMode::Script => {
            // Script mode: execute user's Lua strategy code
            let code = match &strategy.strategy_code {
                Some(c) if !c.is_empty() => c,
                _ => {
                    warn!("Script strategy '{}' has no code", strategy.name);
                    return Ok((None, klines));
                }
            };

            let executor = lua_executor::LuaExecutor::new(lua_executor::LuaExecutorConfig::default());

            // Extract params from indicator_config
            let mut params: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
            if let Some(obj) = strategy.indicator_config.as_object() {
                for (key, value) in obj {
                    if let Some(num) = value.as_f64() {
                        params.insert(key.clone(), num);
                    }
                }
            }

            // Build LuaContext from position info
            let ctx = lua_executor::LuaContext {
                position: lua_executor::LuaPosition {
                    side: position_info
                        .as_ref()
                        .map(|(side, _, _)| side.clone())
                        .unwrap_or_else(|| "flat".to_string()),
                    entry_price: position_info
                        .as_ref()
                        .map(|(_, price, _)| *price)
                        .unwrap_or(0.0),
                    size: position_info
                        .as_ref()
                        .map(|(_, _, size)| *size)
                        .unwrap_or(0.0),
                },
                last_exit_bar: 0,  // TODO: track from position manager
                bar_index: idx as i64,
            };

            match executor.execute(code, &klines, idx, &params, &ctx) {
                Ok(result) => {
                    // Prefer explicit orders, fall back to signal
                    if let Some(order) = result.orders.first() {
                        match order {
                            lua_executor::LuaOrder::Buy { .. } => 1_i8,
                            lua_executor::LuaOrder::Sell { .. } => -1_i8,
                            lua_executor::LuaOrder::Close => {
                                // Close current position — signal depends on current side
                                if let Some((side, _, _)) = &position_info {
                                    match side.as_str() {
                                        "long" => -1_i8,
                                        "short" => 1_i8,
                                        _ => 0_i8,
                                    }
                                } else {
                                    0_i8
                                }
                            }
                        }
                    } else {
                        result.signal.unwrap_or(0)
                    }
                }
                Err(e) => {
                    error!("Lua execution error for strategy '{}': {}", strategy.name, e);
                    0
                }
            }
        }
        StrategyMode::Signal => {
            let plugin_name = strategy
                .indicator_config
                .get("plugin")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    let legacy = strategy.strategy_type.as_str();
                    match legacy {
                        "sma_crossover" | "rsi" | "macd" | "bollinger_bands" => legacy.to_string(),
                        _ => "sma_crossover".to_string(),
                    }
                });

            let mut params: std::collections::HashMap<String, f64> =
                std::collections::HashMap::new();
            if let Some(obj) = strategy.indicator_config.as_object() {
                for (key, value) in obj {
                    if key == "plugin" {
                        continue;
                    }
                    if let Some(num) = value.as_f64() {
                        params.insert(key.clone(), num);
                    }
                }
            }

            if !strategy.indicator_config.get("plugin").is_some() {
                match plugin_name.as_str() {
                    "sma_crossover" => {
                        if let Some(v) = params.remove("short_period") {
                            params.insert("fast_period".into(), v);
                        }
                        if let Some(v) = params.remove("long_period") {
                            params.insert("slow_period".into(), v);
                        }
                    }
                    "macd" => {
                        if let Some(v) = params.remove("fast_period") {
                            params.insert("fast_period".into(), v);
                        }
                        if let Some(v) = params.remove("slow_period") {
                            params.insert("slow_period".into(), v);
                        }
                        if let Some(v) = params.remove("signal_period") {
                            params.insert("signal_period".into(), v);
                        }
                    }
                    _ => {}
                }
            }

            match plugins.generate_signal(&plugin_name, &klines, idx, &params) {
                Ok(signal) => signal,
                Err(e) => {
                    error!(
                        "Strategy '{}' failed to generate signal with plugin '{}': {}",
                        strategy.name, plugin_name, e
                    );
                    0
                }
            }
        }
    };

    // Map raw signal (-1, 0, 1) to SignalType based on trade_direction config
    let signal = match raw_signal {
        0 => None,
        1 if trade_direction == "long" || trade_direction == "both" => Some(SignalType::OpenLong),
        -1 if trade_direction == "short" || trade_direction == "both" => Some(SignalType::OpenShort),
        -1 if trade_direction == "long" => Some(SignalType::CloseLong),
        1 if trade_direction == "short" => Some(SignalType::CloseShort),
        _ => None,
    };

    Ok((signal, klines))
}
