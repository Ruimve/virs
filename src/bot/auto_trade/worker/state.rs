use std::time::Duration;

use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};

use crate::bot::auto_trade::strategy;
use crate::bot::auto_trade::types::AutoEvent;
use crate::bot::auto_trade::worker::AutoWorker;

const MAX_POSITION_DURATION: Duration = Duration::from_secs(48 * 3600);

impl AutoWorker {
    pub async fn run(&mut self, mut shutdown_rx: mpsc::Receiver<()>) {
        info!(
            bot_id = %self.bot.id,
            symbol = %self.bot.symbol,
            market_type = %self.bot.market_type,
            "AutoWorker starting"
        );

        for attempt in 1..=10 {
            self.current_price = self.fetch_current_price().await;
            if self.current_price > 0.0 {
                break;
            }
            warn!(bot_id = %self.bot.id, attempt, "Failed to fetch initial price, retrying in 5s...");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        if self.current_price <= 0.0 {
            error!(bot_id = %self.bot.id, "Failed to fetch initial price after 10 attempts, setting error status");
            let _ = self.store.update_bot_status(self.bot.id, "error").await;
            let _ = self.auto_event_tx.send(AutoEvent::BotError {
                bot_id: self.bot.id,
                error: "Failed to fetch initial price after 10 attempts".to_string(),
            });
            return;
        }
        info!(bot_id = %self.bot.id, price = self.current_price, "Initial price fetched");

        match self.store.load_consecutive_losses(self.bot.id).await {
            Ok(losses) => {
                self.consecutive_losses = losses;
                info!(bot_id = %self.bot.id, consecutive_losses = losses, "Loaded consecutive losses from DB");
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to load consecutive losses, starting from 0");
            }
        }

        if self.bot.current_side.is_none() {
            self.bot.current_side = Some("none".to_string());
        }

        if self.has_position() {
            self.position_opened_at = Some(tokio::time::Instant::now());
            if self.check_stop_take_profit().await {
                info!(bot_id = %self.bot.id, "Stop/take profit triggered on startup, skipping initial LLM analysis");
                self.save_position().await;
            } else {
                info!(bot_id = %self.bot.id, "Performing initial LLM analysis");
                self.on_llm_decision().await;
            }
        } else {
            info!(bot_id = %self.bot.id, "Performing initial LLM analysis");
            self.on_llm_decision().await;
        }

        let mut price_tick = tokio::time::interval(Duration::from_secs(5));

        let (llm_signal_tx, mut llm_signal_rx) = mpsc::channel::<()>(1);
        {
            let interval_secs = self.bot.decide_interval_secs.max(60) as u64;
            info!(bot_id = %self.bot.id, interval_secs, "LLM periodic decision enabled");
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(interval_secs));
                tick.tick().await;
                loop {
                    tick.tick().await;
                    if llm_signal_tx.send(()).await.is_err() {
                        break;
                    }
                }
            });
        }

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!(bot_id = %self.bot.id, "AutoWorker shutting down");
                    break;
                }
                _ = price_tick.tick() => {
                    self.current_price = self.fetch_current_price().await;
                    if !self.paused {
                        self.on_price_tick().await;
                    }
                }
                Some(()) = llm_signal_rx.recv() => {
                    self.on_llm_decision().await;
                }
                event = self.event_rx.recv() => {
                    match event {
                        Ok(event) => self.on_order_event(event).await,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(bot_id = %self.bot.id, lagged = n, "Event lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!(bot_id = %self.bot.id, "Event channel closed");
                            break;
                        }
                    }
                }
            }
        }

        self.save_position().await;
        self.save_stats().await;
    }

    pub(crate) async fn on_price_tick(&mut self) {
        if self.current_price <= 0.0 {
            return;
        }

        self.check_pending_timeout();

        if self.pending_close.is_some() {
            return;
        }

        if self.has_position() {
            self.update_unrealized_pnl();

            let atr = self.fetch_current_atr().await;
            self.update_trailing_stop(atr);

            if self.check_position_timeout().await {
                return;
            }

            if self.check_stop_take_profit().await {
                return;
            }
        }

        if self.trailing_stop_dirty {
            self.save_position().await;
            self.trailing_stop_dirty = false;
        }

        let _ = self.auto_event_tx.send(AutoEvent::PriceUpdate {
            bot_id: self.bot.id,
            price: self.current_price,
        });
    }

    fn update_unrealized_pnl(&mut self) {
        if !self.has_position() || self.bot.entry_price <= 0.0 {
            return;
        }
        let side = self.bot.current_side.as_deref().unwrap_or("none");
        self.bot.unrealized_pnl = match side {
            "long" => (self.current_price - self.bot.entry_price) * self.bot.position_size,
            "short" => (self.bot.entry_price - self.current_price) * self.bot.position_size,
            _ => 0.0,
        };
    }

    async fn check_stop_take_profit(&mut self) -> bool {
        if !self.has_position() || self.bot.entry_price <= 0.0 {
            return false;
        }

        let side = self.bot.current_side.as_deref().unwrap_or("none");
        let should_close = match side {
            "long" => {
                (self.bot.stop_loss > 0.0 && self.current_price <= self.bot.stop_loss)
                    || (self.bot.take_profit > 0.0 && self.current_price >= self.bot.take_profit)
            }
            "short" => {
                (self.bot.stop_loss > 0.0 && self.current_price >= self.bot.stop_loss)
                    || (self.bot.take_profit > 0.0 && self.current_price <= self.bot.take_profit)
            }
            _ => false,
        };

        if should_close {
            let reason = if self.bot.stop_loss > 0.0
                && ((side == "long" && self.current_price <= self.bot.stop_loss)
                    || (side == "short" && self.current_price >= self.bot.stop_loss))
            {
                "stop_loss"
            } else {
                "take_profit"
            };
            info!(bot_id = %self.bot.id, side = %side, reason = %reason, price = self.current_price, "Stop/take profit triggered");
            self.close_position(reason).await;
            return true;
        }
        false
    }

    fn update_trailing_stop(&mut self, atr: f64) {
        if !self.has_position() || self.bot.entry_price <= 0.0 || self.bot.stop_loss <= 0.0 {
            return;
        }

        if atr <= 0.0 {
            return;
        }

        let side = self.bot.current_side.as_deref().unwrap_or("none");

        let new_stop = strategy::compute_trailing_stop(
            self.bot.entry_price,
            self.current_price,
            side,
            atr,
            self.bot.stop_loss,
        );

        if new_stop != self.bot.stop_loss {
            info!(
                bot_id = %self.bot.id, side = %side,
                old_stop = self.bot.stop_loss, new_stop,
                "Trailing stop updated"
            );
            self.bot.stop_loss = new_stop;
            self.trailing_stop_dirty = true;
        }
    }

    async fn fetch_current_atr(&self) -> f64 {
        let snapshot = self.market_data_provider.get_market_snapshot(
            &self.bot.exchange,
            &self.bot.symbol,
            self.bot.market_type.as_str(),
        ).await;
        snapshot.indicators.atr
    }

    async fn check_position_timeout(&mut self) -> bool {
        if let Some(opened_at) = self.position_opened_at {
            if opened_at.elapsed() > MAX_POSITION_DURATION {
                warn!(
                    bot_id = %self.bot.id,
                    duration_secs = opened_at.elapsed().as_secs(),
                    "Position held too long, force closing"
                );
                self.close_position("position_timeout").await;
                return true;
            }
        }
        false
    }

    pub(crate) async fn on_order_event(&mut self, event: crate::bot::auto_trade::ports::OrderEvent) {
        use crate::bot::auto_trade::ports::OrderEvent;

        match event {
            OrderEvent::OrderFilled { order } => {
                if !self.matches_pending_order(order.client_order_id.as_deref()) {
                    return;
                }

                let fill_price = order.fill_price
                    .or(order.request_price)
                    .unwrap_or(self.current_price);
                let filled_qty = if order.filled > 0.0 { order.filled } else { 0.0 };

                if self.pending_open.is_some() {
                    info!(
                        bot_id = %self.bot.id,
                        symbol = %order.symbol,
                        side = ?order.side,
                        fill_price,
                        filled_qty,
                        "Open order filled, confirming position"
                    );
                    self.apply_pending_open(fill_price, filled_qty).await;
                } else if self.pending_close.is_some() {
                    info!(
                        bot_id = %self.bot.id,
                        symbol = %order.symbol,
                        side = ?order.side,
                        fill_price,
                        filled_qty,
                        "Close order filled, confirming close"
                    );
                    self.apply_pending_close(fill_price, filled_qty).await;
                }
            }
            OrderEvent::OrderFailed { order_id: _, reason } => {
                if self.pending_open.is_some() || self.pending_close.is_some() {
                    warn!(
                        bot_id = %self.bot.id,
                        reason = %reason,
                        "Order failed, rolling back pending state"
                    );
                    self.rollback_pending_open();
                    self.rollback_pending_close();
                }
            }
            OrderEvent::LiquidationWarning { symbol, liquidation_price, current_price } => {
                warn!(
                    bot_id = %self.bot.id, symbol = %symbol,
                    liquidation_price, current_price,
                    "Liquidation warning in auto trade"
                );
            }
            _ => {}
        }
    }
}
