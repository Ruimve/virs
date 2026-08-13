

use std::time::Duration;

use tracing::{info, warn};
use virs_type::PositionSide;

use crate::chat::worker::side_str;
use crate::chat::worker::BotWorker;

impl BotWorker {
    pub(crate) async fn on_price_tick(&mut self) {
        /* 价格更新时的风控检查：pending超时 -> 持仓超时 -> 止损止盈触发 */
        if self.current_price <= 0.0 {
            return;
        }

        self.check_pending_timeout().await;

        if self.is_pending() {
            return;
        }

        if self.has_any_position() {
            if self.check_position_timeout().await {
                return;
            }

            if self.check_stop_take_profit().await {
                return;
            }
        }
    }


    /* 止损止盈检查：多头价格<=止损或>=止盈时平仓，空头反之 */
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

    /* 持仓超时检查：超过max_position_duration_secs强制平仓，防止长期持有风险 */
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
