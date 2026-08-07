

use tracing::{error, info, warn};
use uuid::Uuid;
use virs_error::VirsError;

use crate::auto::ports::AutoMarketSnapshot;
use crate::auto::strategy;
use virs_type::OrderEvent;
use virs_type::PositionSide;
use virs_type::EngineEvent;

use crate::auto::worker::{side_str, AutoWorker};

impl AutoWorker {
    pub(crate) async fn on_pe_event(&mut self, event: EngineEvent) {
        /* 处理持仓引擎事件：通过position_id和symbol匹配本bot的持仓，
         * 更新缓存中的持仓状态。外部平仓事件会触发冷却期。 */
        match event {
            EngineEvent::PositionUpdated { position } => {
                if position.symbol != self.bot.symbol {
                    return;
                }
                let side = position.side.clone();
                let cached_id = self.get_position(&side).map(|p| p.id);
                let is_ours = match cached_id {
                    Some(pid) => pid == position.id,
                    None => {
                        let persisted = match side {
                            PositionSide::Long => self.bot.position_id_long,
                            PositionSide::Short => self.bot.position_id_short,
                            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                        };
                        match persisted.filter(|id| *id != Uuid::nil()) {
                            Some(pid) => pid == position.id,
                            None => position.is_open(),
                        }
                    }
                };
                if !is_ours {
                    return;
                }

                let needs_update = match side {
                    PositionSide::Long => {
                        self.bot.position_id_long.is_none()
                            || self.bot.position_id_long == Some(Uuid::nil())
                    }
                    PositionSide::Short => {
                        self.bot.position_id_short.is_none()
                            || self.bot.position_id_short == Some(Uuid::nil())
                    }
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                };
                if needs_update {
                    match side {
                        PositionSide::Long => self.bot.position_id_long = Some(position.id),
                        PositionSide::Short => self.bot.position_id_short = Some(position.id),
                        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                    }
                    if let Err(e) = self
                        .store
                        .update_position(
                            self.bot.id,
                            self.bot.position_id_long,
                            self.bot.position_id_short,
                        )
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                    }
                }
                match side {
                    PositionSide::Long => self.long.position = Some(position),
                    PositionSide::Short => self.short.position = Some(position),
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                }
            }
            EngineEvent::PositionClosed { position } => {
                if position.symbol != self.bot.symbol {
                    return;
                }
                let side = position.side;
                let cached_id = self.get_position(&side).map(|p| p.id);
                let is_ours = match cached_id {
                    Some(pid) => pid == position.id,
                    None => {
                        let persisted = match side {
                            PositionSide::Long => self.bot.position_id_long,
                            PositionSide::Short => self.bot.position_id_short,
                            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                        };
                        match persisted.filter(|id| *id != Uuid::nil()) {
                            Some(pid) => pid == position.id,
                            None => self.get_position(&side).is_some(),
                        }
                    }
                };
                if !is_ours {
                    return;
                }


                let now = chrono::Utc::now();

                self.side_mut(&side).clear_on_close((
                    side_str(&side).to_string(),
                    "external_close".to_string(),
                    now,
                ));
                match side {
                    PositionSide::Long => self.bot.position_id_long = None,
                    PositionSide::Short => self.bot.position_id_short = None,
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                }
                warn!(
                    bot_id = %self.bot.id, side = %side_str(&side),
                    "Position closed by external event (not initiated by worker) — \
                     per-side state cleared, cooldown armed with reason=external_close"
                );
                if let Err(e) = self
                    .store
                    .update_position(
                        self.bot.id,
                        self.bot.position_id_long,
                        self.bot.position_id_short,
                    )
                    .await
                {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                }
            }
            EngineEvent::PositionOpened { position } => {
                if position.symbol != self.bot.symbol {
                    return;
                }

                let needs_update = match position.side {
                    PositionSide::Long => {
                        self.bot.position_id_long.is_none()
                            || self.bot.position_id_long == Some(Uuid::nil())
                    }
                    PositionSide::Short => {
                        self.bot.position_id_short.is_none()
                            || self.bot.position_id_short == Some(Uuid::nil())
                    }
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                };
                if needs_update {
                    match position.side {
                        PositionSide::Long => self.bot.position_id_long = Some(position.id),
                        PositionSide::Short => self.bot.position_id_short = Some(position.id),
                        PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                    }
                    if let Err(e) = self
                        .store
                        .update_position(
                            self.bot.id,
                            self.bot.position_id_long,
                            self.bot.position_id_short,
                        )
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to update position");
                    }
                }
                match position.side {
                    PositionSide::Long => self.long.position = Some(position),
                    PositionSide::Short => self.short.position = Some(position),
                    PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
                }
            }
            _ => {}
        }
    }

    pub(crate) async fn on_order_event(&mut self, event: OrderEvent) {
        /* 处理订单事件：OrderFilled确认开仓/平仓，OrderFailed/OrderCanceled回滚pending状态 */
        match event {
            OrderEvent::OrderFilled { order } => {
                if !self.matches_pending_order(order.client_order_id.as_deref()) {
                    return;
                }

                let order_position_id = order.position_id;

                let fill_price = match order.fill_price.or(order.request_price) {
                    Some(p) => p,
                    None => {
                        error!(
                            order_id = %order.id,
                            error = %VirsError::bad_request("Order has no fill_price and no request_price"),
                            "Skipping order — cannot determine fill price"
                        );
                        return;
                    }
                };
                let filled_qty = if order.filled > 0.0 {
                    order.filled
                } else {
                    0.0
                };

                let cid = match order.client_order_id.as_deref() {
                    Some(cid) => cid,
                    None => return,
                };

                let is_open_long = self
                    .long
                    .pending_open
                    .as_ref()
                    .is_some_and(|p| p.client_order_id == cid);
                let is_open_short = self
                    .short
                    .pending_open
                    .as_ref()
                    .is_some_and(|p| p.client_order_id == cid);
                let is_close_long = self
                    .long
                    .pending_close
                    .as_ref()
                    .is_some_and(|p| p.client_order_id == cid);
                let is_close_short = self
                    .short
                    .pending_close
                    .as_ref()
                    .is_some_and(|p| p.client_order_id == cid);

                if is_open_long {
                    self.apply_pending_open(PositionSide::Long, fill_price, filled_qty, order.fee, order_position_id)
                        .await;
                } else if is_open_short {
                    self.apply_pending_open(PositionSide::Short, fill_price, filled_qty, order.fee, order_position_id)
                        .await;
                } else if is_close_long {
                    self.apply_pending_close(PositionSide::Long, fill_price, filled_qty, order.fee)
                        .await;
                } else if is_close_short {
                    self.apply_pending_close(PositionSide::Short, fill_price, filled_qty, order.fee)
                        .await;
                }
            }
            OrderEvent::OrderFailed {
                order_id: _,
                client_order_id,
                reason,
            } if self.is_pending() => {
                let mut rolled_back_open = false;

                match client_order_id.as_deref() {
                    Some(cid) => {

                        let mut rolled_back_close = false;
                        if self.long.pending_open.as_ref().is_some_and(|p| p.client_order_id == cid) {
                            self.rollback_pending_open(PositionSide::Long);
                            rolled_back_open = true;
                        }
                        if self.short.pending_open.as_ref().is_some_and(|p| p.client_order_id == cid) {
                            self.rollback_pending_open(PositionSide::Short);
                            rolled_back_open = true;
                        }
                        if self.long.pending_close.as_ref().is_some_and(|p| p.client_order_id == cid) {
                            self.rollback_pending_close(PositionSide::Long);
                            rolled_back_close = true;
                        }
                        if self.short.pending_close.as_ref().is_some_and(|p| p.client_order_id == cid) {
                            self.rollback_pending_close(PositionSide::Short);
                            rolled_back_close = true;
                        }

                        if !rolled_back_open && !rolled_back_close {
                            warn!(
                                bot_id = %self.bot.id,
                                client_order_id = %cid,
                                reason = %reason,
                                "OrderFailed received but no matching pending order found — no rollback performed"
                            );
                            return;
                        }
                        warn!(
                            bot_id = %self.bot.id,
                            client_order_id = %cid,
                            reason = %reason,
                            was_open = rolled_back_open,
                            "Order failed, rolling back matching pending state"
                        );
                    }
                    None => {

                        rolled_back_open = self.long.pending_open.is_some()
                            || self.short.pending_open.is_some();
                        warn!(
                            bot_id = %self.bot.id,
                            reason = %reason,
                            was_open = rolled_back_open,
                            "Order failed (no client_order_id), rolling back all pending state"
                        );
                        self.rollback_pending_open(PositionSide::Long);
                        self.rollback_pending_open(PositionSide::Short);
                        self.rollback_pending_close(PositionSide::Long);
                        self.rollback_pending_close(PositionSide::Short);
                    }
                }

                let exec_status = if rolled_back_open {
                    "open_failed"
                } else {
                    "close_failed"
                };
                let log_ids: Vec<Uuid> = [
                    self.long.log_id.take(),
                    self.short.log_id.take(),
                ]
                .into_iter()
                .flatten()
                .collect();
                for log_id in log_ids {
                    if let Err(e) = self
                        .store
                        .update_analysis_log_execution(log_id, exec_status, Some(&reason))
                        .await
                    {
                        error!(bot_id = %self.bot.id, error = %e, "Failed to update log on order failed");
                    }
                }
            }
            OrderEvent::OrderCanceled {
                order_id: _,
                client_order_id,
                symbol: _,
            } if self.is_pending() => {

                let mut rolled_back_open = false;

                match client_order_id.as_deref() {
                    Some(cid) => {
                        let mut rolled_back_close = false;
                        if self.long.pending_open.as_ref().is_some_and(|p| p.client_order_id == cid) {
                            self.rollback_pending_open(PositionSide::Long);
                            rolled_back_open = true;
                        }
                        if self.short.pending_open.as_ref().is_some_and(|p| p.client_order_id == cid) {
                            self.rollback_pending_open(PositionSide::Short);
                            rolled_back_open = true;
                        }
                        if self.long.pending_close.as_ref().is_some_and(|p| p.client_order_id == cid) {
                            self.rollback_pending_close(PositionSide::Long);
                            rolled_back_close = true;
                        }
                        if self.short.pending_close.as_ref().is_some_and(|p| p.client_order_id == cid) {
                            self.rollback_pending_close(PositionSide::Short);
                            rolled_back_close = true;
                        }

                        if !rolled_back_open && !rolled_back_close {
                            warn!(
                                bot_id = %self.bot.id,
                                client_order_id = %cid,
                                "OrderCanceled received but no matching pending order found — no rollback performed"
                            );
                            return;
                        }
                        warn!(
                            bot_id = %self.bot.id,
                            client_order_id = %cid,
                            was_open = rolled_back_open,
                            "Order canceled, rolling back matching pending state"
                        );
                    }
                    None => {
                        rolled_back_open = self.long.pending_open.is_some()
                            || self.short.pending_open.is_some();
                        warn!(
                            bot_id = %self.bot.id,
                            was_open = rolled_back_open,
                            "Order canceled (no client_order_id), rolling back all pending state"
                        );
                        self.rollback_pending_open(PositionSide::Long);
                        self.rollback_pending_open(PositionSide::Short);
                        self.rollback_pending_close(PositionSide::Long);
                        self.rollback_pending_close(PositionSide::Short);
                    }
                }

                let exec_status = if rolled_back_open {
                    "open_canceled"
                } else {
                    "close_canceled"
                };
                let log_ids: Vec<Uuid> = [
                    self.long.log_id.take(),
                    self.short.log_id.take(),
                ]
                .into_iter()
                .flatten()
                .collect();
                for log_id in log_ids {
                    if let Err(e) = self
                        .store
                        .update_analysis_log_execution(log_id, exec_status, Some("order canceled"))
                        .await
                    {
                        error!(bot_id = %self.bot.id, error = %e, "Failed to update log on order canceled");
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) async fn apply_pending_open(
        &mut self,
        side: PositionSide,
        fill_price: f64,
        filled_qty: f64,
        fee: f64,
        order_position_id: Option<Uuid>,
    ) {
        /* 确认开仓成交：若实际成交价与请求价偏差>0.5%则重新计算止损止盈，
         * 更新SideState并记录开仓交易到数据库 */
        let pending = self.side_mut(&side).pending_open.take();
        let Some(pending) = pending else { return };

        let actual_qty = if filled_qty > 0.0 {
            filled_qty
        } else {
            pending.position_size
        };

        let price_deviation = if pending.entry_price > 0.0 {
            (fill_price - pending.entry_price).abs() / pending.entry_price
        } else {
            0.0
        };

        /* 成交价偏差超过0.5%时，基于实际成交价重新计算止损止盈 */
        let (stop_loss, take_profit) = if price_deviation > 0.005 {
            let atr = match self
                .market_data_provider
                .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
                .await
            {
                Ok(s) => match AutoMarketSnapshot::from_base(s) {
                    Ok(snap) => {
                        let atr = snap.indicators
                            .get_num(&virs_indicator::IndicatorSpec::Atr { tf: virs_type::Timeframe::H1, period: 14 })
                            .unwrap_or(0.0);
                        if atr > 0.0 {
                            atr
                        } else {
                            warn!(bot_id = %self.bot.id, "ATR is zero in snapshot, using fill_price * 2% as fallback for SL/TP");
                            fill_price * 0.02
                        }
                    }
                    Err(e) => {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to parse indicators for SL/TP recalculation, using fill_price * 2%");
                        fill_price * 0.02
                    }
                },
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch market snapshot for SL/TP recalculation, using fill_price * 2%");
                    fill_price * 0.02
                }
            };
            let sl = strategy::compute_stop_loss(fill_price, &pending.side, atr);
            let tp = strategy::compute_take_profit(fill_price, &pending.side, atr);
            warn!(
                bot_id = %self.bot.id,
                requested_price = pending.entry_price, fill_price,
                deviation_pct = price_deviation * 100.0,
                old_sl = pending.stop_loss, new_sl = sl,
                old_tp = pending.take_profit, new_tp = tp,
                "Fill price deviated, recalculating stop/take profit"
            );
            (sl, tp)
        } else {
            (pending.stop_loss, pending.take_profit)
        };

        info!(
            bot_id = %self.bot.id, side = %pending.side,
            entry_price = fill_price, requested_qty = pending.position_size,
            actual_qty, stop_loss, take_profit,
            "Open order confirmed, applying position state"
        );


        {
            let s = self.side_mut(&side);
            s.stop_loss = stop_loss;
            s.take_profit = take_profit;
            s.position_opened_at = Some(tokio::time::Instant::now());
            s.open_fee = fee;
        }


        if let Some(pid) = order_position_id.filter(|id| *id != Uuid::nil()) {
            match side {
                PositionSide::Long => {
                    if self.bot.position_id_long.filter(|id| *id != Uuid::nil()).is_none() {
                        self.bot.position_id_long = Some(pid);
                    }
                }
                PositionSide::Short => {
                    if self.bot.position_id_short.filter(|id| *id != Uuid::nil()).is_none() {
                        self.bot.position_id_short = Some(pid);
                    }
                }
                PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
            }
        }

        self.save_position().await;

        let trade_type = match pending.side.as_str() {
            "long" => "open_long",
            "short" => "open_short",
            _ => {
                error!(bot_id = %self.bot.id, side = %pending.side, "Unknown pending side — skipping trade record");
                return;
            }
        };

        let client_order_id = pending.client_order_id.clone();
        match self
            .store
            .record_open_trade(
                self.bot.id,
                self.bot.user_id,
                &self.bot.symbol,
                &self.bot.exchange,
                &client_order_id,
                stop_loss,
                take_profit,
                &self.bot.strategy_file,
            )
            .await
        {
            Ok(()) => {
                info!(bot_id = %self.bot.id, client_order_id = %client_order_id, trade_type, stop_loss, take_profit, "Open trade recorded");
                self.side_mut(&side).open_client_order_id = Some(client_order_id);
            }
            Err(e) => {
                error!(bot_id = %self.bot.id, error = %e, "Failed to record open trade");
            }
        }

        if actual_qty < pending.position_size {
            warn!(
                bot_id = %self.bot.id,
                requested = pending.position_size,
                filled = actual_qty,
                "Partial open: position opened with less than requested quantity"
            );
        }

        let log_id = self.side_mut(&side).log_id.take();
        if let Some(log_id) = log_id {
            if let Err(e) = self
                .store
                .update_analysis_log_execution(log_id, "open", None)
                .await
            {
                error!(bot_id = %self.bot.id, error = %e, "Failed to update open execution status");
            }
        }
    }

    pub(crate) async fn apply_pending_close(
        &mut self,
        side: PositionSide,
        fill_price: f64,
        filled_qty: f64,
        fee: f64,
    ) {
        /* 确认平仓成交：计算已实现盈亏（毛利-总手续费），更新统计数据，
         * 清理SideState，记录平仓事件和冷却期，关闭交易记录 */
        let pending = self.side_mut(&side).pending_close.take();
        let Some(pending) = pending else { return };

        let actual_qty = if filled_qty > 0.0 {
            filled_qty
        } else {
            pending.position_size
        };

        let gross_pnl = match pending.side.as_str() {
            "long" => (fill_price - pending.entry_price) * actual_qty,
            "short" => (pending.entry_price - fill_price) * actual_qty,
            _ => {
                error!(bot_id = %self.bot.id, side = %pending.side, "Unknown pending side — skipping trade record");
                return;
            }
        };
        let open_fee = self.side(&side).open_fee;
        let total_fee = open_fee + fee;
        let realized_pnl = gross_pnl - total_fee;

        let pnl_pct = if pending.entry_price > 0.0 && actual_qty > 0.0 {
            realized_pnl / (pending.entry_price * actual_qty) * 100.0
        } else {
            0.0
        };

        info!(
            bot_id = %self.bot.id, side = %pending.side,
            entry_price = pending.entry_price, close_price = fill_price,
            quantity = actual_qty, realized_pnl, pnl_pct,
            close_reason = %pending.close_reason,
            open_fee = open_fee, close_fee = fee, total_fee,
            "Position closed"
        );

        self.bot.total_pnl += realized_pnl;
        self.bot.total_trades += 1;
        if realized_pnl >= 0.0 {
            self.bot.win_trades += 1;
            self.consecutive_losses = 0;
        } else {
            self.bot.loss_trades += 1;
            self.consecutive_losses += 1;
        }


        {
            let s = self.side_mut(&side);
            s.stop_loss = 0.0;
            s.take_profit = 0.0;
            s.position = None;
            s.position_opened_at = None;
            s.open_fee = 0.0;
        }


        let close_event = (
            pending.side.clone(),
            pending.close_reason.clone(),
            chrono::Utc::now(),
        );
        self.side_mut(&side).last_close_event = Some(close_event);


        match side {
            PositionSide::Long => self.bot.position_id_long = None,
            PositionSide::Short => self.bot.position_id_short = None,
            PositionSide::Unknown(_) => unreachable!("validate ensures position_side is Long/Short"),
        }

        self.save_position().await;
        self.save_stats().await;

        let close_reason = &pending.close_reason;
        let close_client_order_id = pending.client_order_id.clone();

        let open_client_order_id = self.side_mut(&side).open_client_order_id.take();
        match open_client_order_id {
            Some(open_oid) => {
                if let Err(e) = self
                    .store
                    .close_trade(&open_oid, &close_client_order_id, close_reason)
                    .await
                {
                    error!(bot_id = %self.bot.id, open_client_order_id = %open_oid, error = %e, "Failed to close trade record");
                } else {
                    info!(bot_id = %self.bot.id, open_client_order_id = %open_oid, realized_pnl, "Close trade recorded");
                }
            }
            None => match self.store.find_open_trade(self.bot.id).await {
                Ok(Some((open_oid, _sl, _tp, _opened_at))) => {
                    if let Err(e) = self
                        .store
                        .close_trade(&open_oid, &close_client_order_id, close_reason)
                        .await
                    {
                        error!(bot_id = %self.bot.id, open_client_order_id = %open_oid, error = %e, "Failed to close trade record (recovered)");
                    } else {
                        info!(bot_id = %self.bot.id, open_client_order_id = %open_oid, "Close trade recorded (recovered from DB)");
                    }
                }
                Ok(None) => {
                    warn!(bot_id = %self.bot.id, "No open trade found for close, recording as orphaned");
                    if let Err(e) = self
                        .store
                        .record_orphaned_close_trade(
                            self.bot.id,
                            self.bot.user_id,
                            &self.bot.symbol,
                            &self.bot.exchange,
                            &close_client_order_id,
                            close_reason,
                            &self.bot.strategy_file,
                        )
                        .await
                    {
                        warn!(bot_id = %self.bot.id, error = %e, "Failed to record orphaned close trade");
                    }
                }
                Err(e) => {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to find open trade for close");
                }
            },
        }

        let log_id = self.side_mut(&side).log_id.take();
        if let Some(log_id) = log_id {
            if let Err(e) = self
                .store
                .update_analysis_log_execution(log_id, "close", None)
                .await
            {
                error!(bot_id = %self.bot.id, error = %e, "Failed to update close execution status");
            }
        }
    }

    fn rollback_pending_open(&mut self, side: PositionSide) {
        let pending = self.side_mut(&side).pending_open.take();
        if pending.is_some() {
            warn!(bot_id = %self.bot.id, side = %side_str(&side), "Rolling back pending open order");
        }
    }

    fn rollback_pending_close(&mut self, side: PositionSide) {
        let pending = self.side_mut(&side).pending_close.take();
        if pending.is_some() {
            warn!(bot_id = %self.bot.id, side = %side_str(&side), "Rolling back pending close order");
        }
    }
}
