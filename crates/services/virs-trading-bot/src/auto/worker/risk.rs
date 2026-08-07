

use std::time::Duration;

use tracing::{info, warn};
use virs_type::PositionSide;

use crate::auto::ports::AutoMarketSnapshot;
use crate::auto::strategy;
use crate::auto::worker::side_str;
use crate::auto::worker::AutoWorker;

impl AutoWorker {
    pub(crate) async fn on_price_tick(&mut self) {
        if self.current_price <= 0.0 {
            return;
        }

        self.check_pending_timeout().await;

        if self.is_pending() {
            return;
        }

        if self.has_any_position() {
            if let Some(atr) = self.fetch_current_atr().await {
                self.update_trailing_stop(atr).await;
            }

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
    }


    pub(crate) async fn check_stop_take_profit(&mut self) -> bool {
        let mut triggered = false;
        if self.has_position_side(PositionSide::Long)
            && self.check_stop_take_profit_side(PositionSide::Long).await {
                triggered = true;
            }
        if self.has_position_side(PositionSide::Short)
            && self.check_stop_take_profit_side(PositionSide::Short).await {
                triggered = true;
            }
        triggered
    }

    async fn check_stop_take_profit_side(&mut self, side: PositionSide) -> bool {
        let entry_price = match self.get_position(&side) {
            Some(p) if p.is_open() => p.entry_price,
            _ => return false,
        };
        if entry_price <= 0.0 {
            return false;
        }

        let s = self.side(&side);
        let stop_loss = s.stop_loss;
        let take_profit = s.take_profit;
        let side_str = side_str(&side);

        let should_close = match side {
            PositionSide::Long => {
                (stop_loss > 0.0 && self.current_price <= stop_loss)
                    || (take_profit > 0.0 && self.current_price >= take_profit)
            }
            PositionSide::Short => {
                (stop_loss > 0.0 && self.current_price >= stop_loss)
                    || (take_profit > 0.0 && self.current_price <= take_profit)
            }
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        };

        if should_close {
            let _stop_triggered = stop_loss > 0.0
                && ((side == PositionSide::Long && self.current_price <= stop_loss)
                    || (side == PositionSide::Short && self.current_price >= stop_loss));
            let take_triggered = take_profit > 0.0
                && ((side == PositionSide::Long && self.current_price >= take_profit)
                    || (side == PositionSide::Short && self.current_price <= take_profit));

            let close_reason = if take_triggered {
                "take_profit"
            } else {
                "stop_loss"
            };
            info!(
                bot_id = %self.bot.id, side = %side_str,
                close_reason = %close_reason, price = self.current_price,
                stop_loss, take_profit,
                "Stop/take profit triggered"
            );
            self.close_position(side, close_reason).await;
            return true;
        }
        false
    }

    async fn update_trailing_stop(&mut self, atr: f64) {
        if atr <= 0.0 {
            return;
        }
        self.update_trailing_stop_side(PositionSide::Long, atr).await;
        self.update_trailing_stop_side(PositionSide::Short, atr).await;
    }

    async fn update_trailing_stop_side(&mut self, side: PositionSide, atr: f64) {
        let entry_price = match self.get_position(&side) {
            Some(p) if p.is_open() => p.entry_price,
            _ => return,
        };

        let s = self.side(&side);
        let stop_loss = s.stop_loss;
        let client_order_id = s.open_client_order_id.clone();

        if entry_price <= 0.0 || stop_loss <= 0.0 {
            return;
        }

        let side_str = side_str(&side);

        let new_stop = strategy::compute_trailing_stop(
            entry_price,
            self.current_price,
            side_str,
            atr,
            stop_loss,
        );

        if new_stop != stop_loss {
            self.side_mut(&side).stop_loss = new_stop;
            self.trailing_stop_dirty = true;

            if let Some(client_order_id) = client_order_id {


                if let Err(e) = self
                    .store
                    .update_trade_stop_loss(&client_order_id, new_stop)
                    .await
                {
                    warn!(client_order_id = %client_order_id, error = %e, "Failed to update trade stop_loss");
                }
            }
        }
    }

    async fn fetch_current_atr(&self) -> Option<f64> {
        let snapshot = match self
            .market_data_provider
            .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
            .await
        {
            Ok(s) => match AutoMarketSnapshot::from_base(s) {
                Ok(snap) => snap,
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to parse indicators for ATR");
                    return None;
                }
            },
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch market snapshot for ATR");
                return None;
            }
        };
        Some(snapshot.indicators.get_num(&virs_indicator::IndicatorSpec::Atr { tf: virs_type::Timeframe::H1, period: 14 }).unwrap_or(0.0))
    }

    async fn check_position_timeout(&mut self) -> bool {
        let mut triggered = false;
        if self.check_position_timeout_side(PositionSide::Long).await {
            triggered = true;
        }
        if self.check_position_timeout_side(PositionSide::Short).await {
            triggered = true;
        }
        triggered
    }

    async fn check_position_timeout_side(&mut self, side: PositionSide) -> bool {
        let max_duration = Duration::from_secs(self.time_config.max_position_duration_secs);
        let opened_at = self.side(&side).position_opened_at;
        if let Some(opened_at) = opened_at {
            if opened_at.elapsed() > max_duration {
                warn!(
                    bot_id = %self.bot.id,
                    side = %side_str(&side),
                    duration_secs = opened_at.elapsed().as_secs(),
                    "Position held too long, force closing"
                );
                self.close_position(side, "position_timeout").await;
                return true;
            }
        }
        false
    }
}
