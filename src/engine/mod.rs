pub mod backtest;
pub mod position;

use crate::models::*;
use crate::exchange::Exchange;
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
    ) -> Self {
        Self {
            strategies: Arc::new(DashMap::new()),
            exchange_instances: Arc::new(DashMap::new()),
            order_tx,
            config,
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
        if let Some(mut entry) = self.strategies.get_mut(strategy_id) {
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
/// Signal mapping depends on the strategy's `allow_short` trading config flag:
/// - `allow_short = true`: signal 1 → OpenLong, -1 → OpenShort (when flat) or CloseLong/CloseShort (when in position)
/// - `allow_short = false` (default): signal 1 → OpenLong, -1 → CloseLong
///
/// This enables both long-only and long/short strategies from the same indicator logic.
async fn run_strategy_cycle(
    exchange: &dyn Exchange,
    symbol: &str,
    timeframe: &str,
    strategy: &Strategy,
) -> anyhow::Result<Option<SignalType>> {
    let klines = exchange.get_klines(symbol, timeframe, 200, None).await?;

    if klines.len() < 50 {
        return Ok(None);
    }

    let idx = klines.len() - 1;

    let indicator_type = strategy
        .indicator_config
        .get("indicator")
        .and_then(|v| v.as_str())
        .unwrap_or("sma_crossover");

    // Check if short selling is allowed for this strategy
    let allow_short = strategy
        .trading_config
        .get("allow_short")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let raw_signal = match indicator_type {
        "sma_crossover" => {
            let fast = strategy.indicator_config.get("fast_period").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
            let slow = strategy.indicator_config.get("slow_period").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            crate::engine::backtest::sma_crossover_signal(&klines, idx, fast, slow)
        }
        "rsi" => {
            let period = strategy.indicator_config.get("period").and_then(|v| v.as_u64()).unwrap_or(14) as usize;
            let oversold = strategy.indicator_config.get("oversold").and_then(|v| v.as_f64()).unwrap_or(30.0);
            let overbought = strategy.indicator_config.get("overbought").and_then(|v| v.as_f64()).unwrap_or(70.0);
            crate::engine::backtest::rsi_signal(&klines, idx, period, oversold, overbought)
        }
        "macd" => {
            let fast = strategy.indicator_config.get("fast_period").and_then(|v| v.as_u64()).unwrap_or(12) as usize;
            let slow = strategy.indicator_config.get("slow_period").and_then(|v| v.as_u64()).unwrap_or(26) as usize;
            let signal_period = strategy.indicator_config.get("signal_period").and_then(|v| v.as_u64()).unwrap_or(9) as usize;
            crate::engine::backtest::macd_signal(&klines, idx, fast, slow, signal_period)
        }
        "bollinger_bands" => {
            let period = strategy.indicator_config.get("period").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let std_dev = strategy.indicator_config.get("std_dev").and_then(|v| v.as_f64()).unwrap_or(2.0);
            crate::engine::backtest::bollinger_bands_signal(&klines, idx, period, std_dev)
        }
        _ => {
            warn!("Unknown indicator type: {}", indicator_type);
            return Ok(None);
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
