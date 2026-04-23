//! Spot market strategy engine.

pub mod position;

use async_trait::async_trait;
use crate::models::*;
use crate::exchange::Exchange;
use crate::engine::base::MarketEngine;
use crate::engine::plugin::PluginRegistry;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use uuid::Uuid;

pub struct SpotMarketEngine {
    strategies: Arc<DashMap<Uuid, RunningStrategy>>,
    order_tx: mpsc::Sender<super::OrderCommand>,
    plugins: Arc<PluginRegistry>,
    ws_broadcaster: Option<Arc<crate::api::ws::WsBroadcaster>>,
    db_pool: Option<sqlx::PgPool>,
}

#[derive(Debug)]
struct RunningStrategy {
    strategy: Strategy,
    cancel_token: tokio::sync::watch::Sender<bool>,
}

impl SpotMarketEngine {
    pub fn new(
        order_tx: mpsc::Sender<super::OrderCommand>,
        plugins: Arc<PluginRegistry>,
    ) -> Self {
        Self {
            strategies: Arc::new(DashMap::new()),
            order_tx,
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
}

#[async_trait]
impl MarketEngine for SpotMarketEngine {
    fn market_type(&self) -> MarketType {
        MarketType::Spot
    }

    fn running_strategy_ids(&self) -> Vec<Uuid> {
        self.strategies.iter().map(|r| *r.key()).collect()
    }

    fn is_running(&self, strategy_id: &Uuid) -> bool {
        self.strategies.contains_key(strategy_id)
    }

    fn stop_strategy(&self, strategy_id: &Uuid) -> bool {
        if let Some(entry) = self.strategies.get_mut(strategy_id) {
            let name = entry.strategy.name.clone();
            let _ = entry.cancel_token.send(true);
            info!("Stopping spot strategy {}", strategy_id);

            self.emit_event(crate::api::ws::WsEvent::StrategyStatus {
                strategy_id: strategy_id.to_string(),
                name,
                status: "stopped".to_string(),
            });

            self.strategies.remove(strategy_id);
            true
        } else {
            false
        }
    }

    async fn start_strategy(
        &self,
        strategy: Strategy,
        exchange_name: String,
        exchanges: Arc<DashMap<String, Box<dyn Exchange>>>,
    ) -> anyhow::Result<()> {
        let strategy_id = strategy.id;
        let symbol = strategy.symbol.clone();
        let timeframe = strategy.timeframe.clone();
        let interval_secs = strategy.decide_interval_secs;
        let trading_config = strategy.trading_config.clone();
        let strategies = self.strategies.clone();
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
            "Starting spot strategy {} ({} on {} / {})",
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
                    warn!("Spot strategy {} failed to restore position: {}", strategy.name, e);
                }
            }

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Some(exchange) = exchanges.get(&exchange_name) {
                            let current_price = match exchange.get_ticker(&symbol).await {
                                Ok(ticker) => ticker.last,
                                Err(e) => {
                                    warn!("Spot strategy {} failed to get ticker: {}", strategy.name, e);
                                    continue;
                                }
                            };

                            if current_price <= 0.0 {
                                warn!("Spot strategy {} got invalid ticker price {}", strategy.name, current_price);
                                continue;
                            }

                            pos_manager.update_price_tracking(current_price);

                            if let Some((close_side, close_amount, risk_reason)) = pos_manager.check_risk(current_price) {
                                info!(
                                    "Spot strategy {} risk management triggered close: {:?} for {} @ {:.2}",
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
                                let _ = order_tx.send(super::OrderCommand::Place {
                                    strategy_id,
                                    symbol: symbol.clone(),
                                    signal_type: signal_type.clone(),
                                    side: close_side,
                                    amount: close_amount,
                                    price: None,
                                    order_type: OrderType::Market,
                                    exchange_name: exchange_name.clone(),
                                    market_type: MarketType::Spot,
                                    reduce_only: false,
                                    position_side: None,
                                    callback: cb_tx,
                                }).await;

                                match tokio::time::timeout(
                                    std::time::Duration::from_secs(30),
                                    cb_rx,
                                ).await {
                                    Ok(Ok(super::OrderResult::Filled { fill_price, filled_amount, .. })) => {
                                        info!(
                                            "Spot strategy {} risk close confirmed: fill_price={:.2}, filled={:.6}",
                                            strategy.name, fill_price, filled_amount
                                        );
                                        pos_manager.apply_risk_close();
                                    }
                                    Ok(Ok(super::OrderResult::Failed { error })) => {
                                        error!(
                                            "Spot strategy {} risk close FAILED: {}. Will retry next cycle.",
                                            strategy.name, error
                                        );
                                    }
                                    Ok(Err(_)) => {
                                        error!(
                                            "Spot strategy {} risk close callback dropped. Will retry next cycle.",
                                            strategy.name
                                        );
                                    }
                                    Err(_) => {
                                        error!(
                                            "Spot strategy {} risk close timed out (30s). Will retry next cycle.",
                                            strategy.name
                                        );
                                    }
                                }
                                continue;
                            }

                            let position_info = match pos_manager.position_info() {
                                Some(p) => {
                                    match p.side {
                                        PositionSide::Long => super::CyclePositionInfo::long_only(p.entry_price, p.size),
                                        PositionSide::Short => super::CyclePositionInfo::short_only(p.entry_price, p.size),
                                    }
                                }
                                None => super::CyclePositionInfo::flat(),
                            };

                            match super::run_strategy_cycle(
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
                                                "Spot strategy {} generated signal: {:?} for {}",
                                                strategy.name, signal, symbol
                                            );

                                            let (side, base_amount) = pos_manager
                                                .prepare_order_async(&signal, &*exchange, current_price)
                                                .await;

                                            if base_amount <= 0.0 {
                                                warn!(
                                                    "Spot strategy {} signal {:?} produced amount=0, skipping",
                                                    strategy.name, signal
                                                );
                                                continue;
                                            }

                                            let (cb_tx, cb_rx) = tokio::sync::oneshot::channel();
                                            let _ = order_tx.send(super::OrderCommand::Place {
                                                strategy_id,
                                                symbol: symbol.clone(),
                                                signal_type: signal.clone(),
                                                side: side.clone(),
                                                amount: base_amount,
                                                price: None,
                                                order_type: OrderType::Market,
                                                exchange_name: exchange_name.clone(),
                                                market_type: MarketType::Spot,
                                                reduce_only: false,
                                                position_side: None,
                                                callback: cb_tx,
                                            }).await;

                                            match tokio::time::timeout(
                                                std::time::Duration::from_secs(30),
                                                cb_rx,
                                            ).await {
                                                Ok(Ok(super::OrderResult::Filled { fill_price, filled_amount, .. })) => {
                                                    info!(
                                                        "Spot strategy {} order confirmed: fill_price={:.2}, filled={:.6}",
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
                                                Ok(Ok(super::OrderResult::Failed { error })) => {
                                                    error!(
                                                        "Spot strategy {} order FAILED: {}. Position state NOT updated.",
                                                        strategy.name, error
                                                    );
                                                }
                                                Ok(Err(_)) => {
                                                    error!(
                                                        "Spot strategy {} order callback dropped. Position state NOT updated.",
                                                        strategy.name
                                                    );
                                                }
                                                Err(_) => {
                                                    warn!(
                                                        "Spot strategy {} order timed out (30s). Position state NOT updated - will rely on next cycle's position check.",
                                                        strategy.name
                                                    );
                                                }
                                            }
                                        }
                                        None => {
                                            tracing::debug!("Spot strategy {} no signal", strategy.name);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Spot strategy {} error: {}", strategy.name, e);
                                }
                            }
                        } else {
                            warn!("Exchange {} not found for spot strategy {}", exchange_name, strategy.name);
                        }
                    }
                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            info!("Spot strategy {} stopped", strategy.name);
                            break;
                        }
                    }
                }
            }
        });

        Ok(())
    }
}

async fn restore_position_from_db(
    pos_manager: &mut position::PositionManager,
    pool: &sqlx::PgPool,
    strategy_id: Uuid,
) -> anyhow::Result<()> {
    let row: Option<(String, f64, f64)> = sqlx::query_as(
        r#"SELECT trade_type, price, amount
           FROM qd_strategy_trades
           WHERE strategy_id = $1 AND trade_type IN ('OpenLong', 'CloseLong')
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .bind(strategy_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB query failed: {}", e))?;

    if let Some((trade_type, price, amount)) = &row {
        if trade_type == "OpenLong" {
            pos_manager.restore_position(crate::models::PositionSide::Long, *amount, *price);
        } else {
            info!(
                "Spot strategy {} last trade is CloseLong, no position to restore",
                strategy_id
            );
        }
    } else {
        info!("Spot strategy {} has no trade history, starting fresh", strategy_id);
    }

    Ok(())
}
