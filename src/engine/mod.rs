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

pub struct StrategyEngine {
    strategies: Arc<DashMap<Uuid, RunningStrategy>>,
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
struct RunningStrategy {
    strategy: Strategy,
    cancel_token: tokio::sync::watch::Sender<bool>,
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
            strategies: Arc::new(DashMap::new()),
            exchange_instances: Arc::new(DashMap::new()),
            order_tx,
            config,
            plugins,
            ws_broadcaster: None,
            db_pool: None,
        }
    }

    pub fn set_db_pool(&mut self, pool: sqlx::PgPool) {
        self.db_pool = Some(pool);
    }

    pub fn set_ws_broadcaster(&mut self, broadcaster: Arc<crate::api::ws::WsBroadcaster>) {
        self.ws_broadcaster = Some(broadcaster);
    }

    fn emit_event(&self, event: crate::api::ws::WsEvent) {
        if let Some(ref broadcaster) = self.ws_broadcaster {
            let _ = broadcaster.send(event);
        }
    }

    pub fn register_exchange(&self, exchange: Box<dyn Exchange>) {
        let name = exchange.name().to_string();
        info!("Registered exchange: {}", name);
        self.exchange_instances.insert(name, exchange);
    }

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

    pub fn get_exchange(&self, name: &str) -> Option<dashmap::mapref::one::Ref<'_, String, Box<dyn Exchange>>> {
        self.exchange_instances.get(name)
    }

    pub fn registered_exchange_names(&self) -> Vec<String> {
        self.exchange_instances.iter().map(|r| r.key().clone()).collect()
    }

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
        let cancel_tx_clone = cancel_tx.clone();

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

        self.emit_event(crate::api::ws::WsEvent::StrategyStatus {
            strategy_id: strategy_id.to_string(),
            name: strategy.name.clone(),
            status: "running".to_string(),
        });

        let ws_broadcaster = self.ws_broadcaster.clone();
        let db_pool = self.db_pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                interval_secs as u64,
            ));

            let mut pos_manager = position::PositionManager::new(
                strategy_id,
                symbol.clone(),
                exchange_name.clone(),
                &trading_config,
            );

            if let Some(ref pool) = db_pool {
                if let Err(e) = restore_position_from_db(&mut pos_manager, pool, strategy_id).await {
                    warn!("Strategy {} failed to restore position: {}", strategy.name, e);
                }
            }

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Some(exchange) = exchanges.get(&exchange_name) {
                            let current_price = match exchange.get_ticker(&symbol).await {
                                Ok(ticker) => ticker.last,
                                Err(e) => {
                                    warn!("Strategy {} failed to get ticker: {}", strategy.name, e);
                                    continue;
                                }
                            };

                            if current_price <= 0.0 {
                                warn!("Strategy {} got invalid ticker price {}", strategy.name, current_price);
                                continue;
                            }

                            pos_manager.update_price_tracking(current_price);

                            if let Some((close_side, close_amount, risk_reason)) = pos_manager.check_risk(current_price) {
                                info!(
                                    "Strategy {} risk management triggered close: {:?} for {} @ {:.2}",
                                    strategy.name, close_side, symbol, current_price
                                );

                                if let Some(ref bc) = ws_broadcaster {
                                    let _ = bc.send(crate::api::ws::WsEvent::Risk {
                                        strategy_id: strategy_id.to_string(),
                                        symbol: symbol.clone(),
                                        reason: risk_reason.as_str().to_string(),
                                        price: current_price,
                                    });
                                }

                                let signal_type = if close_side == Side::Sell {
                                    SignalType::CloseLong
                                } else {
                                    SignalType::CloseShort
                                };

                                let (cb_tx, cb_rx) = tokio::sync::oneshot::channel();
                                let _ = order_tx.send(OrderCommand::Place {
                                    strategy_id,
                                    symbol: symbol.clone(),
                                    signal_type: signal_type.clone(),
                                    side: close_side,
                                    amount: close_amount,
                                    price: None,
                                    order_type: OrderType::Market,
                                    exchange_name: exchange_name.clone(),
                                    callback: cb_tx,
                                }).await;

                                match tokio::time::timeout(
                                    std::time::Duration::from_secs(30),
                                    cb_rx,
                                ).await {
                                    Ok(Ok(OrderResult::Filled { fill_price, filled_amount, .. })) => {
                                        info!(
                                            "Strategy {} risk close confirmed: fill_price={:.2}, filled={:.6}",
                                            strategy.name, fill_price, filled_amount
                                        );
                                        pos_manager.apply_risk_close();
                                    }
                                    Ok(Ok(OrderResult::Failed { error })) => {
                                        error!(
                                            "Strategy {} risk close FAILED: {}. Will retry next cycle.",
                                            strategy.name, error
                                        );
                                    }
                                    Ok(Err(_)) => {
                                        error!(
                                            "Strategy {} risk close callback dropped. Will retry next cycle.",
                                            strategy.name
                                        );
                                    }
                                    Err(_) => {
                                        error!(
                                            "Strategy {} risk close timed out (30s). Will retry next cycle.",
                                            strategy.name
                                        );
                                    }
                                }
                                continue;
                            }

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
                                Ok((signal, _klines)) => {
                                    match signal {
                                        Some(signal) => {
                                            info!(
                                                "Strategy {} generated signal: {:?} for {}",
                                                strategy.name, signal, symbol
                                            );

                                            let (side, base_amount) = pos_manager
                                                .prepare_order_async(&signal, &*exchange, current_price)
                                                .await;

                                            if base_amount <= 0.0 {
                                                warn!(
                                                    "Strategy {} signal {:?} produced amount=0, skipping",
                                                    strategy.name, signal
                                                );
                                                continue;
                                            }

                                            let (cb_tx, cb_rx) = tokio::sync::oneshot::channel();
                                            let _ = order_tx.send(OrderCommand::Place {
                                                strategy_id,
                                                symbol: symbol.clone(),
                                                signal_type: signal.clone(),
                                                side: side.clone(),
                                                amount: base_amount,
                                                price: None,
                                                order_type: OrderType::Market,
                                                exchange_name: exchange_name.clone(),
                                                callback: cb_tx,
                                            }).await;

                                            match tokio::time::timeout(
                                                std::time::Duration::from_secs(30),
                                                cb_rx,
                                            ).await {
                                                Ok(Ok(OrderResult::Filled { fill_price, filled_amount, .. })) => {
                                                    info!(
                                                        "Strategy {} order confirmed: fill_price={:.2}, filled={:.6}",
                                                        strategy.name, fill_price, filled_amount
                                                    );
                                                    pos_manager.apply_signal(&signal, filled_amount, fill_price);

                                                    if let Some(ref bc) = ws_broadcaster {
                                                        let side_str = match side {
                                                            Side::Buy => "buy",
                                                            Side::Sell => "sell",
                                                        };
                                                        let _ = bc.send(crate::api::ws::WsEvent::Trade {
                                                            strategy_id: strategy_id.to_string(),
                                                            symbol: symbol.clone(),
                                                            side: side_str.to_string(),
                                                            price: fill_price,
                                                            amount: filled_amount,
                                                            pnl: 0.0,
                                                        });
                                                    }
                                                }
                                                Ok(Ok(OrderResult::Failed { error })) => {
                                                    error!(
                                                        "Strategy {} order FAILED: {}. Position state NOT updated.",
                                                        strategy.name, error
                                                    );
                                                }
                                                Ok(Err(_)) => {
                                                    error!(
                                                        "Strategy {} order callback dropped. Position state NOT updated.",
                                                        strategy.name
                                                    );
                                                }
                                                Err(_) => {
                                                    error!(
                                                        "Strategy {} order timed out (30s). Querying exchange for order status...",
                                                        strategy.name
                                                    );
                                                    let open_orders = match exchange.get_open_orders(Some(&symbol)).await {
                                                        Ok(orders) => orders,
                                                        Err(e) => {
                                                            error!("Strategy {} failed to query open orders: {}", strategy.name, e);
                                                            continue;
                                                        }
                                                    };
                                                    if open_orders.is_empty() {
                                                        info!(
                                                            "Strategy {} no open orders found after timeout - order likely filled. Pausing strategy to prevent duplicate positions.",
                                                            strategy.name
                                                        );
                                                        let _ = cancel_tx_clone.send(true);
                                                        break;
                                                    } else {
                                                        for open_order in &open_orders {
                                                            if let Err(e) = exchange.cancel_order(&symbol, &open_order.id).await {
                                                                warn!("Strategy {} failed to cancel order {}: {}", strategy.name, open_order.id, e);
                                                            }
                                                        }
                                                        info!(
                                                            "Strategy {} cancelled {} open orders after timeout. Position state NOT updated.",
                                                            strategy.name, open_orders.len()
                                                        );
                                                    }
                                                }
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

    pub fn stop_strategy(&self, strategy_id: &Uuid) -> bool {
        if let Some(entry) = self.strategies.get_mut(strategy_id) {
            let name = entry.strategy.name.clone();
            let _ = entry.cancel_token.send(true);
            info!("Stopping strategy {}", strategy_id);

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

    pub fn running_strategy_ids(&self) -> Vec<Uuid> {
        self.strategies.iter().map(|r| *r.key()).collect()
    }

    pub fn is_running(&self, strategy_id: &Uuid) -> bool {
        self.strategies.contains_key(strategy_id)
    }
}

async fn run_strategy_cycle(
    exchange: &dyn Exchange,
    symbol: &str,
    timeframe: &str,
    strategy: &Strategy,
    plugins: &PluginRegistry,
    position_info: Option<(String, f64, f64)>,
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

    let position_side: Option<&str> = position_info.as_ref().map(|(side, _, _)| side.as_str());

    let signal = map_raw_signal(raw_signal, trade_direction, position_side);

    Ok((signal, klines))
}

pub fn map_raw_signal(
    raw_signal: i8,
    trade_direction: &str,
    current_position_side: Option<&str>,
) -> Option<SignalType> {
    if raw_signal == 0 {
        return None;
    }

    let is_flat = current_position_side.is_none() || current_position_side == Some("flat");
    let is_long = current_position_side == Some("long");
    let is_short = current_position_side == Some("short");

    match (raw_signal, trade_direction, is_flat, is_long, is_short) {
        (1, "long", true, _, _) => Some(SignalType::OpenLong),
        (-1, "long", _, true, _) => Some(SignalType::CloseLong),

        (-1, "short", true, _, _) => Some(SignalType::OpenShort),
        (1, "short", _, _, true) => Some(SignalType::CloseShort),

        (1, "both", true, _, _) => Some(SignalType::OpenLong),
        (1, "both", _, _, true) => Some(SignalType::CloseShort),
        (-1, "both", true, _, _) => Some(SignalType::OpenShort),
        (-1, "both", _, true, _) => Some(SignalType::CloseLong),

        _ => None,
    }
}

async fn restore_position_from_db(
    pos_manager: &mut position::PositionManager,
    pool: &sqlx::PgPool,
    strategy_id: Uuid,
) -> anyhow::Result<()> {
    let row: Option<(String, f64, f64)> = sqlx::query_as(
        r#"SELECT side, price, amount
           FROM qd_strategy_trades
           WHERE strategy_id = $1
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .bind(strategy_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB query failed: {}", e))?;

    if let Some((side_str, price, amount)) = row {
        let side = match side_str.to_lowercase().as_str() {
            "buy" | "openlong" => crate::models::PositionSide::Long,
            "sell" | "openshort" => crate::models::PositionSide::Short,
            _ => {
                info!(
                    "Strategy {} last trade is a close (side={}), no position to restore",
                    strategy_id, side_str
                );
                return Ok(());
            }
        };
        pos_manager.restore_position(side, amount, price);
    } else {
        info!("Strategy {} has no trade history, starting fresh", strategy_id);
    }

    Ok(())
}
