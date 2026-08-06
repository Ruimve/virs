//! Order execution: open and close position commands.

use tracing::{error, info, warn};
use uuid::Uuid;

use crate::auto::ports::AutoMarketSnapshot;
use crate::auto::strategy;
use virs_type::OrderCommand;
use crate::auto::worker::client_order_id::{format_auto_open, format_auto_close};
use virs_type::{PositionSide, Side};

use crate::auto::worker::{side_str, AutoWorker, PendingClose, PendingOpen};

impl AutoWorker {
    pub(super) async fn open_position(
        &mut self,
        side: &str,
        snapshot: &AutoMarketSnapshot,
    ) {
        let account = match self
            .market_data_provider
            .get_account_balance(&self.bot.exchange)
            .await
        {
            Ok(b) => b,
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch account balance, skipping open");
                return;
            }
        };

        if account.total <= 0.0 && account.free <= 0.0 {
            warn!(bot_id = %self.bot.id, "Account balance is zero, skipping open");
            return;
        }

        let atr = snapshot.indicators
            .get_num(&virs_indicator::IndicatorSpec::Atr { tf: virs_type::Timeframe::H1, period: 14 })
            .filter(|&v| v > 0.0)
            .unwrap_or(snapshot.base.current_price * 0.02);
        let adx = snapshot.indicators
            .get_num(&virs_indicator::IndicatorSpec::Adx { tf: virs_type::Timeframe::H1, period: 14 })
            .unwrap_or(0.0);
        let funding_rate = snapshot.base.funding_rate;
        let price = snapshot.base.current_price;

        let position_size_pct =
            strategy::compute_position_pct(adx, self.consecutive_losses, funding_rate)
                .min(self.bot.max_position_pct);

        let invest_amount = account.free * 0.95 * position_size_pct / 100.0;
        if invest_amount < 1.0 {
            warn!(bot_id = %self.bot.id, invest_amount, "Insufficient funds for opening position");
            return;
        }

        let quantity = invest_amount * self.bot.leverage as f64 / price;

        let min_qty = snapshot.base.min_qty;
        let quantity = if min_qty > 0.0 && quantity < min_qty {
            warn!(
                bot_id = %self.bot.id,
                quantity, min_qty, invest_amount, price,
                "Quantity below min_qty, using min_qty instead"
            );
            min_qty
        } else if min_qty > 0.0 {
            (quantity / min_qty).floor() * min_qty
        } else {
            quantity
        };

        let stop_loss = strategy::compute_stop_loss(price, side, atr);
        let take_profit = strategy::compute_take_profit(price, side, atr);
        let sl_source = "formula";
        let tp_source = "formula";

        let position_side = match side {
            "long" => PositionSide::Long,
            "short" => PositionSide::Short,
            _ => {
                error!(bot_id = %self.bot.id, side = %side, "Unknown position side — refusing to place order");
                return;
            }
        };

        let order_side = match side {
            "long" => Side::Buy,
            "short" => Side::Sell,
            _ => {
                error!(bot_id = %self.bot.id, side = %side, "Unknown position side — refusing to place order");
                return;
            }
        };

        let client_order_id = format_auto_open(self.bot.id, side);

        let result = self
            .order_executor
            .send_command(OrderCommand::OpenPosition {
                symbol: self.bot.symbol.clone(),
                side: position_side,
                order_side,
                amount: quantity,
                leverage: self.bot.leverage.max(1) as u32,
                price: None,
                client_order_id: Some(client_order_id.clone()),
            })
            .await;

        match result {
            Ok(()) => {
                info!(
                    bot_id = %self.bot.id, side = %side,
                    price, quantity,
                    stop_loss, take_profit,
                    sl_source, tp_source,
                    "Position opening order sent, awaiting confirmation"
                );

                let pending = PendingOpen {
                    side: side.to_string(),
                    entry_price: price,
                    position_size: quantity,
                    stop_loss,
                    take_profit,
                    client_order_id,
                    sent_at: tokio::time::Instant::now(),
                };
                match side {
                    "long" => self.long.pending_open = Some(pending),
                    "short" => self.short.pending_open = Some(pending),
                    _ => unreachable!(),
                }
            }
            Err(e) => {
                error!(bot_id = %self.bot.id, client_order_id = %client_order_id, symbol = %self.bot.symbol, error = %e, "Failed to send open position order");
            }
        }
    }

    pub(crate) async fn close_position(&mut self, side: PositionSide, close_reason: &str) {
        let position = match self.get_position(&side) {
            Some(p) if p.is_open() => p.clone(),
            _ => return,
        };

        let side_str = side_str(&side);
        let entry_price = position.entry_price;
        let position_size = position.quantity;
        let position_id = position.id;

        // 优先使用 per-side 缓存的 position_id；若为 nil 则回退到 PlaceOrder 路径
        if position_id != Uuid::nil() {
            let client_order_id = format_auto_close(self.bot.id, side_str);

            let result = self
                .order_executor
                .send_command(OrderCommand::ClosePosition {
                    position_id,
                    price: None,
                    client_order_id: Some(client_order_id.clone()),
                })
                .await;

            match result {
                Ok(()) => {
                    info!(
                        bot_id = %self.bot.id, side = %side_str,
                        entry_price = entry_price,
                        close_price = self.current_price,
                        close_reason = %close_reason,
                        "Position closing order sent via ClosePosition, awaiting confirmation"
                    );

                    let pending = PendingClose {
                        side: side_str.to_string(),
                        close_reason: close_reason.to_string(),
                        entry_price,
                        position_size,
                        client_order_id,
                        sent_at: tokio::time::Instant::now(),
                    };
                    match side {
                        PositionSide::Long => self.long.pending_close = Some(pending),
                        PositionSide::Short => self.short.pending_close = Some(pending),
                        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                    }
                }
                Err(e) => {
                    error!(bot_id = %self.bot.id, client_order_id = %client_order_id, symbol = %self.bot.symbol, side = %side_str, error = %e, "Failed to send close position order");
                }
            }
        } else {
            // 回退路径：缓存 position_id 为 nil 时使用 PlaceOrder 反向单
            let (order_side, position_side_field) = match side {
                PositionSide::Long => (Side::Sell, Some(PositionSide::Long)),
                PositionSide::Short => (Side::Buy, Some(PositionSide::Short)),
                PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
            };

            let client_order_id = format_auto_close(self.bot.id, side_str);

            let result = self
                .order_executor
                .send_command(OrderCommand::PlaceOrder {
                    symbol: self.bot.symbol.clone(),
                    side: order_side,
                    amount: position_size,
                    price: None,
                    position_side: position_side_field,
                    position_id: None,
                    client_order_id: Some(client_order_id.clone()),
                })
                .await;

            match result {
                Ok(()) => {
                    info!(
                        bot_id = %self.bot.id, side = %side_str,
                        entry_price = entry_price,
                        close_price = self.current_price,
                        close_reason = %close_reason,
                        "Position closing order sent via PlaceOrder, awaiting confirmation"
                    );

                    let pending = PendingClose {
                        side: side_str.to_string(),
                        close_reason: close_reason.to_string(),
                        entry_price,
                        position_size,
                        client_order_id,
                        sent_at: tokio::time::Instant::now(),
                    };
                    match side {
                        PositionSide::Long => self.long.pending_close = Some(pending),
                        PositionSide::Short => self.short.pending_close = Some(pending),
                        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                    }
                }
                Err(e) => {
                    error!(bot_id = %self.bot.id, client_order_id = %client_order_id, symbol = %self.bot.symbol, side = %side_str, error = %e, "Failed to send close position order (fallback path)");
                }
            }
        }
    }
}
