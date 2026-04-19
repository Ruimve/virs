pub mod backtest;
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
        }
    }

    /// Register an exchange instance.
    pub fn register_exchange(&self, exchange: Box<dyn Exchange>) {
        let name = exchange.name().to_string();
        info!("Registered exchange: {}", name);
        self.exchange_instances.insert(name, exchange);
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
    pub async fn start_strategy(&self, strategy: Strategy) -> anyhow::Result<()> {
        let strategy_id = strategy.id;
        let exchange_name = strategy.exchange.clone();
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

        // Spawn strategy task
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
                            match run_strategy_cycle(
                                &*exchange,
                                &symbol,
                                &timeframe,
                                &strategy,
                                &plugins,
                            ).await {
                                Ok(Some(signal)) => {
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

                                    let _ = order_tx.send(OrderCommand::Place {
                                        strategy_id,
                                        symbol: symbol.clone(),
                                        signal_type: signal.clone(),
                                        side,
                                        amount: order_amount,
                                        price: None,
                                        order_type: OrderType::Market,
                                        exchange_name: exchange_name.clone(),
                                    }).await;
                                }
                                Ok(None) => {
                                    tracing::debug!("Strategy {} no signal", strategy.name);
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
            let _ = entry.cancel_token.send(true);
            info!("Stopping strategy {}", strategy_id);
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
/// Signal mapping depends on the strategy's `allow_short` trading config flag:
/// - `allow_short = true`: signal 1 -> OpenLong, -1 -> OpenShort (when flat) or CloseLong/CloseShort (when in position)
/// - `allow_short = false` (default): signal 1 -> OpenLong, -1 -> CloseLong
///
/// This enables both long-only and long/short strategies from the same indicator logic.
async fn run_strategy_cycle(
    exchange: &dyn Exchange,
    symbol: &str,
    timeframe: &str,
    strategy: &Strategy,
    plugins: &PluginRegistry,
) -> anyhow::Result<Option<SignalType>> {
    let klines = exchange.get_klines(symbol, timeframe, 200, None).await?;

    if klines.len() < 50 {
        return Ok(None);
    }

    let idx = klines.len() - 1;

    // Check if short selling is allowed for this strategy
    let allow_short = strategy
        .trading_config
        .get("allow_short")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Generate signal based on strategy mode
    let raw_signal: i8 = match strategy.strategy_mode {
        StrategyMode::Script => {
            // Script mode: execute user's Lua strategy code
            let code = match &strategy.strategy_code {
                Some(c) if !c.is_empty() => c,
                _ => {
                    warn!("Script strategy '{}' has no code", strategy.name);
                    return Ok(None);
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

            match executor.execute(code, &klines, idx, &params) {
                Ok(signal) => signal,
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

    // Map raw signal (-1, 0, 1) to SignalType based on allow_short config
    let signal = match raw_signal {
        0 => None,
        1 => Some(SignalType::OpenLong),
        -1 if allow_short => Some(SignalType::OpenShort),
        -1 => Some(SignalType::CloseLong),
        _ => None,
    };

    Ok(signal)
}
