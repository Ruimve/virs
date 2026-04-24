pub mod backtest;
pub mod base;
pub mod indicators;
pub mod lua_executor;
pub mod perpetual;
pub mod plugin;
pub mod plugins;
pub mod spot;

use crate::models::*;
use crate::exchange::Exchange;
use crate::engine::base::MarketEngine;
use crate::engine::plugin::PluginRegistry;
use crate::engine::spot::SpotMarketEngine;
use crate::engine::perpetual::PerpetualMarketEngine;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use uuid::Uuid;

pub struct StrategyEngine {
    spot_engine: SpotMarketEngine,
    perpetual_engine: PerpetualMarketEngine,
    exchange_instances: Arc<DashMap<String, Box<dyn Exchange>>>,
    order_tx: mpsc::Sender<OrderCommand>,
    config: StrategyEngineConfig,
    plugins: Arc<PluginRegistry>,
    ws_broadcaster: Option<Arc<crate::api::ws::WsBroadcaster>>,
    db_pool: Option<sqlx::PgPool>,
}

#[derive(Debug, Clone)]
pub struct StrategyEngineConfig {
    pub executor_workers: usize,
    pub pending_order_poll_interval_secs: u64,
    pub auto_restore: bool,
}

#[derive(Debug)]
pub enum OrderResult {
    Filled {
        order_id: String,
        fill_price: f64,
        filled_amount: f64,
        fee: f64,
    },
    Failed {
        error: String,
    },
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
        market_type: MarketType,
        reduce_only: bool,
        position_side: Option<PositionSide>,
        callback: tokio::sync::oneshot::Sender<OrderResult>,
    },
    Cancel {
        strategy_id: Uuid,
        symbol: String,
        order_id: String,
        exchange_name: String,
    },
    Query {
        symbol: String,
        order_id: String,
        exchange_name: String,
        callback: tokio::sync::oneshot::Sender<OrderResult>,
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
            spot_engine: SpotMarketEngine::new(order_tx.clone(), plugins.clone()),
            perpetual_engine: PerpetualMarketEngine::new(order_tx.clone(), plugins.clone()),
            exchange_instances: Arc::new(DashMap::new()),
            order_tx,
            config,
            plugins,
            ws_broadcaster: None,
            db_pool: None,
        }
    }

    pub fn set_db_pool(&mut self, pool: sqlx::PgPool) {
        self.db_pool = Some(pool.clone());
        self.spot_engine.set_db_pool(pool.clone());
        self.perpetual_engine.set_db_pool(pool);
    }

    pub fn set_ws_broadcaster(&mut self, broadcaster: Arc<crate::api::ws::WsBroadcaster>) {
        self.ws_broadcaster = Some(broadcaster.clone());
        self.spot_engine.set_ws_broadcaster(broadcaster.clone());
        self.perpetual_engine.set_ws_broadcaster(broadcaster);
    }

    pub fn register_exchange(&self, exchange: Box<dyn Exchange>) {
        let name = exchange.name().to_string();
        let mt = exchange.market_type();
        let key = format!("{}:{}", name, mt);
        info!("Registered exchange: {} (key={})", name, key);
        self.exchange_instances.insert(key, exchange);
    }

    pub fn register_exchange_for_user(
        &self,
        exchange: Box<dyn Exchange>,
        user_id: Uuid,
    ) -> String {
        let raw_name = exchange.name().to_string();
        let mt = exchange.market_type();
        let scoped_name = format!("{}:{}:{}", raw_name, mt, user_id);
        info!("Registered exchange '{}' ({:?}) for user {}", raw_name, mt, user_id);
        self.exchange_instances.insert(scoped_name.clone(), exchange);
        scoped_name
    }

    pub fn get_exchange(&self, name: &str) -> Option<dashmap::mapref::one::Ref<'_, String, Box<dyn Exchange>>> {
        self.exchange_instances.get(name)
    }

    pub fn registered_exchange_names(&self) -> Vec<String> {
        self.exchange_instances.iter().map(|r| r.key().clone()).collect()
    }

    pub fn remove_user_exchange(&self, exchange_name: &str, market_type: &str, user_id: &str) {
        let key = format!("{}:{}:{}", exchange_name, market_type, user_id);
        self.exchange_instances.remove(&key);
    }

    /// Dispatch strategy start to the appropriate market engine based on strategy.market_type.
    pub async fn start_strategy(&self, strategy: Strategy, exchange_key: Option<String>) -> anyhow::Result<()> {
        let exchange_name = exchange_key.unwrap_or_else(|| {
            let mt_str = match strategy.market_type {
                MarketType::Spot => "spot",
                MarketType::Perpetual => "perpetual",
            };
            format!("{}:{}", strategy.exchange, mt_str)
        });

        match strategy.market_type {
            MarketType::Spot => {
                self.spot_engine.start_strategy(
                    strategy,
                    exchange_name,
                    self.exchange_instances.clone(),
                ).await
            }
            MarketType::Perpetual => {
                self.perpetual_engine.start_strategy(
                    strategy,
                    exchange_name,
                    self.exchange_instances.clone(),
                ).await
            }
        }
    }

    pub fn stop_strategy(&self, strategy_id: &Uuid) -> bool {
        if self.spot_engine.stop_strategy(strategy_id) {
            return true;
        }
        if self.perpetual_engine.stop_strategy(strategy_id) {
            return true;
        }
        false
    }

    pub fn running_strategy_ids(&self) -> Vec<Uuid> {
        let mut ids = self.spot_engine.running_strategy_ids();
        ids.extend(self.perpetual_engine.running_strategy_ids());
        ids
    }

    pub fn is_running(&self, strategy_id: &Uuid) -> bool {
        self.spot_engine.is_running(strategy_id) || self.perpetual_engine.is_running(strategy_id)
    }
}

// ============================================================
// Shared strategy cycle logic
// ============================================================

pub struct CyclePositionInfo {
    pub has_long: bool,
    pub has_short: bool,
    pub primary_side: Option<String>,
    pub primary_entry_price: f64,
    pub primary_size: f64,
}

impl CyclePositionInfo {
    pub fn flat() -> Self {
        Self {
            has_long: false,
            has_short: false,
            primary_side: None,
            primary_entry_price: 0.0,
            primary_size: 0.0,
        }
    }

    pub fn long_only(entry_price: f64, size: f64) -> Self {
        Self {
            has_long: true,
            has_short: false,
            primary_side: Some("long".to_string()),
            primary_entry_price: entry_price,
            primary_size: size,
        }
    }

    pub fn short_only(entry_price: f64, size: f64) -> Self {
        Self {
            has_long: false,
            has_short: true,
            primary_side: Some("short".to_string()),
            primary_entry_price: entry_price,
            primary_size: size,
        }
    }

    pub fn hedge(long_entry: f64, long_size: f64, short_entry: f64, short_size: f64) -> Self {
        Self {
            has_long: true,
            has_short: true,
            primary_side: Some("long".to_string()),
            primary_entry_price: long_entry,
            primary_size: long_size,
        }
    }
}

pub(crate) async fn run_strategy_cycle(
    exchange: &dyn Exchange,
    symbol: &str,
    timeframe: &str,
    strategy: &Strategy,
    plugins: &PluginRegistry,
    position_info: CyclePositionInfo,
) -> anyhow::Result<(Option<SignalType>, Vec<Kline>)> {
    let klines = exchange.get_klines(symbol, timeframe, 200, None).await?;

    if klines.len() < 50 {
        return Ok((None, klines));
    }

    let idx = klines.len() - 1;

    let trade_direction = strategy
        .trading_config
        .get("trade_direction")
        .and_then(|v| v.as_str())
        .unwrap_or("long");

    let raw_signal: i8 = match strategy.strategy_mode {
        StrategyMode::Script => {
            let code = match &strategy.strategy_code {
                Some(c) if !c.is_empty() => c,
                _ => {
                    warn!("Script strategy '{}' has no code", strategy.name);
                    return Ok((None, klines));
                }
            };

            let executor = lua_executor::LuaExecutor::new(lua_executor::LuaExecutorConfig::default());

            let mut params: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
            if let Some(obj) = strategy.indicator_config.as_object() {
                for (key, value) in obj {
                    if let Some(num) = value.as_f64() {
                        params.insert(key.clone(), num);
                    }
                }
            }

            let ctx = lua_executor::LuaContext {
                position: lua_executor::LuaPosition {
                    side: position_info
                        .primary_side
                        .clone()
                        .unwrap_or_else(|| "flat".to_string()),
                    entry_price: position_info.primary_entry_price,
                    size: position_info.primary_size,
                },
                last_exit_bar: 0,
                bar_index: idx as i64,
            };

            match executor.execute(code, &klines, idx, &params, &ctx) {
                Ok(result) => {
                    if let Some(order) = result.orders.first() {
                        match order {
                            lua_executor::LuaOrder::Buy { .. } => 1_i8,
                            lua_executor::LuaOrder::Sell { .. } => -1_i8,
                            lua_executor::LuaOrder::Close => {
                                if position_info.has_long && position_info.has_short {
                                    0_i8
                                } else if position_info.has_long {
                                    -1_i8
                                } else if position_info.has_short {
                                    1_i8
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
                    "dual_ema_trend".to_string()
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

            // Fetch auxiliary klines for plugin's required timeframes
            let required_tfs = plugins.get_required_timeframes(&plugin_name);
            let mut extra_klines_map: HashMap<String, Vec<Kline>> = HashMap::new();
            for tf in &required_tfs {
                if let Ok(tf_klines) = exchange.get_klines(symbol, tf, 200, None).await {
                    if !tf_klines.is_empty() {
                        extra_klines_map.insert(tf.clone(), tf_klines);
                    }
                }
            }

            // Align auxiliary klines to current bar time
            let current_time = klines[idx].open_time;
            let mut aligned: HashMap<String, &[Kline]> = HashMap::new();
            for (tf, tf_klines) in &extra_klines_map {
                let end = tf_klines
                    .iter()
                    .position(|k| k.open_time > current_time)
                    .unwrap_or(tf_klines.len());
                aligned.insert(tf.clone(), &tf_klines[..end]);
            }

            let ctx = plugin::SignalContext {
                klines: &klines,
                idx,
                extra_klines: aligned,
            };

            match plugins.generate_signal(&plugin_name, &ctx, &params) {
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

    let signal = map_raw_signal(raw_signal, trade_direction, position_info.has_long, position_info.has_short);

    Ok((signal, klines))
}

/// Map raw signal (+1/-1/0) to concrete SignalType based on trade direction and current position state.
///
/// Design decisions:
/// - "long": only trades in long direction. Signal 1 = OpenLong, -1 = CloseLong
/// - "short": only trades in short direction. Signal -1 = OpenShort, 1 = CloseShort
/// - "both": trades in both directions with hedge support
///   - When flat (no position): 1 = OpenLong, -1 = OpenShort
///   - When long only: -1 = CloseLong, 1 = CloseShort (open hedge)
///   - When short only: 1 = CloseShort, -1 = CloseLong (open hedge)
///   - When hedged (both): 1 = CloseShort, -1 = CloseLong (close hedge)
///   - When same-direction signal (e.g., long + signal 1): intentionally ignored (no pyramiding)
pub fn map_raw_signal(
    raw_signal: i8,
    trade_direction: &str,
    has_long: bool,
    has_short: bool,
) -> Option<SignalType> {
    if raw_signal == 0 {
        return None;
    }

    let is_flat = !has_long && !has_short;

    match (raw_signal, trade_direction, has_long, has_short) {
        (1, "long", false, _) => Some(SignalType::OpenLong),
        (-1, "long", true, _) => Some(SignalType::CloseLong),

        (-1, "short", _, false) => Some(SignalType::OpenShort),
        (1, "short", _, true) => Some(SignalType::CloseShort),

        (1, "both", false, false) => Some(SignalType::OpenLong),
        (-1, "both", false, false) => Some(SignalType::OpenShort),
        (1, "both", true, true) => Some(SignalType::CloseShort),
        (-1, "both", true, true) => Some(SignalType::CloseLong),
        (1, "both", false, true) => Some(SignalType::CloseShort),
        (-1, "both", true, false) => Some(SignalType::CloseLong),

        _ => None,
    }
}
