use tracing::{info, warn};

use crate::bot::auto_trade::ai::{AutoAction, AutoDecision};
use crate::bot::auto_trade::ports::*;
use crate::bot::auto_trade::strategy::{self, PromptContext};
use crate::bot::auto_trade::types::AutoEvent;
use crate::bot::auto_trade::worker::{AutoWorker, PendingClose, PendingOpen};

impl AutoWorker {
    pub(crate) async fn on_llm_decision(&mut self) {
        info!(bot_id = %self.bot.id, "LLM decision tick");

        if self.is_pending() {
            warn!(bot_id = %self.bot.id, "Pending order in progress, skipping LLM decision");
            return;
        }

        if !self.ai_service.is_available_for_user(&self.bot.user_id).await {
            warn!(bot_id = %self.bot.id, "AI service not available, skipping decision");
            let _ = self.auto_event_tx.send(AutoEvent::BotError {
                bot_id: self.bot.id,
                error: "LLM decision skipped: AI service not configured".to_string(),
            });
            return;
        }

        let (system_prompt, user_prompt) = match self.build_llm_prompt().await {
            Some(prompts) => prompts,
            None => return,
        };

        let decision_result = self.ai_service.auto_decision(&self.bot.user_id, &system_prompt, &user_prompt).await;
        let (decision, raw_llm_response) = match decision_result {
            Some((d, raw)) => (Some(d), Some(raw)),
            None => (None, None),
        };

        let action = self.handle_llm_result(&decision, &system_prompt, &user_prompt, raw_llm_response.as_ref()).await;

        self.execute_decision(&action, decision.as_ref()).await;

        if !matches!(action, AutoAction::Hold) {
            let _ = self.store.update_last_decided(self.bot.id).await;
        }
    }

    async fn build_llm_prompt(&self) -> Option<(String, String)> {
        let snapshot = self.market_data_provider.get_market_snapshot(
            &self.bot.exchange,
            &self.bot.symbol,
            self.bot.market_type.as_str(),
        ).await;

        if snapshot.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping decision");
            return None;
        }

        let account = self.market_data_provider.get_account_balance(
            &self.bot.exchange,
            self.bot.market_type.as_str(),
        ).await;
        let margin_usage_rate = if account.total > 0.0 { account.used / account.total } else { 0.0 };

        let position_info = strategy::format_position_info(
            self.bot.current_side.as_deref(),
            self.bot.entry_price,
            self.bot.position_size,
            snapshot.current_price,
            snapshot.liquidation_price,
        );

        let stop_take_profit_info = strategy::format_stop_take_profit(
            self.bot.stop_loss,
            self.bot.take_profit,
        );

        let position_duration = if self.has_position() {
            if let Some(opened_at) = self.position_opened_at {
                let elapsed = opened_at.elapsed();
                let hours = elapsed.as_secs() / 3600;
                let mins = (elapsed.as_secs() % 3600) / 60;
                format!("{}小时{}分钟", hours, mins)
            } else {
                "未知".to_string()
            }
        } else {
            "无持仓".to_string()
        };

        let ctx = PromptContext {
            timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            symbol: self.bot.symbol.clone(),
            exchange: self.bot.exchange.clone(),
            market_type: self.bot.market_type.as_str().to_string(),
            total_balance: account.total,
            available_balance: account.free,
            used_margin: account.used,
            margin_usage_rate,
            leverage: self.bot.leverage,
            position_info,
            position_duration,
            stop_take_profit_info,
            funding_rate: snapshot.funding_rate,
            funding_next_time: snapshot.funding_next_time,
            total_trades: self.bot.total_trades,
            win_trades: self.bot.win_trades,
            loss_trades: self.bot.loss_trades,
            total_pnl: self.bot.total_pnl,
            consecutive_losses: self.consecutive_losses,
            trigger_reason: "scheduled".to_string(),
            ind: snapshot.indicators,
            min_qty: snapshot.min_qty,
        };

        let template = crate::bot::auto_trade::types::DEFAULT_USER_PROMPT_TEMPLATE;
        let user_prompt = strategy::render_prompt(template, &ctx);
        let system_prompt = self.bot.system_prompt.as_deref()
            .unwrap_or(crate::bot::auto_trade::types::DEFAULT_SYSTEM_PROMPT)
            .to_string();

        Some((system_prompt, user_prompt))
    }

    async fn handle_llm_result(
        &mut self,
        decision: &Option<AutoDecision>,
        system_prompt: &str,
        user_prompt: &str,
        raw_llm_response: Option<&serde_json::Value>,
    ) -> AutoAction {
        match decision {
            Some(d) => {
                info!(bot_id = %self.bot.id, action = d.action.as_str(), reason = %d.reason, confidence = d.confidence, "LLM decision");

                if let Some(ref w) = d.funding_rate_warning {
                    warn!(bot_id = %self.bot.id, warning = %w, "Funding rate warning");
                }
                if let Some(ref w) = d.risk_warning {
                    warn!(bot_id = %self.bot.id, warning = %w, "Risk warning");
                }

                let mut result = serde_json::json!({
                    "decision": {
                        "action": d.action.as_str(),
                        "reason": d.reason,
                        "confidence": d.confidence,
                    },
                    "market": {
                        "market_regime": d.market_regime,
                        "funding_rate_warning": d.funding_rate_warning,
                        "event_impact": d.event_impact,
                    },
                    "analysis": d.analysis,
                    "risk_warning": d.risk_warning,
                });
                if let Some(raw) = raw_llm_response {
                    result.as_object_mut().unwrap().insert("raw_llm_response".to_string(), raw.clone());
                }
                let _ = self.store.save_analysis_log(
                    self.bot.id, "periodic", system_prompt, user_prompt,
                    &result, None,
                ).await;

                d.action.clone()
            }
            None => {
                warn!(bot_id = %self.bot.id, "LLM call failed, holding position");

                let result = serde_json::json!({
                    "decision": {
                        "action": "hold",
                        "reason": "LLM call failed",
                        "confidence": 0.0,
                    },
                    "market": {
                        "market_regime": null,
                        "funding_rate_warning": null,
                        "event_impact": null,
                    },
                    "analysis": null,
                    "risk_warning": null,
                });
                let _ = self.store.save_analysis_log(
                    self.bot.id, "periodic", system_prompt, user_prompt,
                    &result, Some("LLM call failed"),
                ).await;

                let _ = self.auto_event_tx.send(AutoEvent::BotError {
                    bot_id: self.bot.id,
                    error: "LLM call failed, holding".to_string(),
                });

                AutoAction::Hold
            }
        }
    }

    pub(crate) async fn execute_decision(&mut self, action: &AutoAction, decision: Option<&AutoDecision>) {
        if matches!(action, AutoAction::Hold) {
            info!(bot_id = %self.bot.id, "Hold: no action taken, no params applied");
            return;
        }

        if self.is_pending() {
            warn!(bot_id = %self.bot.id, "Pending order in progress, skipping decision execution");
            return;
        }

        if matches!(action, AutoAction::OpenLong | AutoAction::OpenShort) {
            if let Some(d) = decision {
                if d.confidence < 0.6 {
                    warn!(
                        bot_id = %self.bot.id,
                        action = action.as_str(),
                        confidence = d.confidence,
                        "Confidence below 0.6 threshold for opening position, downgrading to Hold"
                    );
                    return;
                }
            }
        }

        let snapshot = self.market_data_provider.get_market_snapshot(
            &self.bot.exchange,
            &self.bot.symbol,
            self.bot.market_type.as_str(),
        ).await;

        if snapshot.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping decision execution");
            return;
        }

        match action {
            AutoAction::OpenLong | AutoAction::OpenShort => {
                if let Some(d) = decision {
                    self.apply_non_structural_params(d, &snapshot).await;
                }

                let side = match action {
                    AutoAction::OpenLong => "long",
                    AutoAction::OpenShort => "short",
                    _ => unreachable!(),
                };

                if side == "short" && self.is_spot() {
                    warn!(bot_id = %self.bot.id, "Cannot open short on spot market");
                    return;
                }
                if self.has_position() {
                    warn!(bot_id = %self.bot.id, side = %side, "Already has position, cannot open");
                    return;
                }
                self.open_position(side, decision, &snapshot).await;
            }
            AutoAction::ClosePosition => {
                if !self.has_position() {
                    warn!(bot_id = %self.bot.id, "No position to close");
                    return;
                }
                let reason = decision
                    .and_then(|d| d.close_reason.as_deref())
                    .unwrap_or("llm_decision");
                self.close_position(reason).await;
            }
            AutoAction::Hold => unreachable!(),
        }
    }

    async fn apply_non_structural_params(&mut self, d: &AutoDecision, snapshot: &MarketSnapshot) {
        if let Some(ref regime) = d.market_regime {
            self.bot.market_regime = Some(regime.clone());
        }

        let adx = snapshot.indicators.adx;
        let new_leverage = strategy::compute_leverage(adx, self.is_spot());
        self.bot.leverage = new_leverage;

        let _ = self.store.update_ai_analysis(
            self.bot.id,
            self.bot.market_regime.as_deref().unwrap_or("ranging"),
            self.bot.leverage,
            d.analysis.as_deref().unwrap_or(""),
        ).await;
    }

    async fn open_position(&mut self, side: &str, decision: Option<&AutoDecision>, snapshot: &MarketSnapshot) {
        let account = self.market_data_provider.get_account_balance(
            &self.bot.exchange,
            self.bot.market_type.as_str(),
        ).await;

        if account.total <= 0.0 && account.free <= 0.0 {
            warn!(bot_id = %self.bot.id, "Cannot retrieve account balance, skipping open");
            let _ = self.auto_event_tx.send(AutoEvent::BotError {
                bot_id: self.bot.id,
                error: format!("Cannot retrieve account balance for opening {} position", side),
            });
            return;
        }

        let atr = if snapshot.indicators.atr > 0.0 { snapshot.indicators.atr } else { snapshot.current_price * 0.02 };
        let adx = snapshot.indicators.adx;
        let funding_rate = snapshot.funding_rate;
        let price = snapshot.current_price;

        let position_size_pct = strategy::compute_position_pct(adx, self.consecutive_losses, funding_rate)
            .min(self.bot.max_position_pct);

        let invest_amount = account.free * 0.95 * position_size_pct / 100.0;
        if invest_amount < 1.0 {
            warn!(bot_id = %self.bot.id, invest_amount, "Insufficient funds for opening position");
            let _ = self.auto_event_tx.send(AutoEvent::BotError {
                bot_id: self.bot.id,
                error: format!("Insufficient funds: only {:.2} USDT available (need >= 1.0)", invest_amount),
            });
            return;
        }

        let quantity = if self.is_spot() {
            invest_amount / price
        } else {
            invest_amount * self.bot.leverage as f64 / price
        };

        let stop_loss = strategy::compute_stop_loss(price, side, atr);

        let take_profit = strategy::compute_take_profit(price, side, atr);

        let position_side = match side {
            "long" => Some(PositionSide::Long),
            "short" => Some(PositionSide::Short),
            _ => None,
        };

        let order_side = match side {
            "long" => OrderSide::Buy,
            "short" => OrderSide::Sell,
            _ => OrderSide::Buy,
        };

        let client_order_id = format!("auto:{}:{}", side, self.bot.id);

        let result = self.order_executor.send_command(OrderCommand::PlaceOrder {
            symbol: self.bot.symbol.clone(),
            side: order_side,
            amount: quantity,
            price: None,
            reduce_only: false,
            position_side,
            client_order_id: Some(client_order_id.clone()),
        }).await;

        match result {
            Ok(()) => {
                info!(
                    bot_id = %self.bot.id, side = %side,
                    price, quantity,
                    stop_loss, take_profit,
                    "Position opening order sent, awaiting confirmation"
                );

                self.pending_open = Some(PendingOpen {
                    side: side.to_string(),
                    entry_price: price,
                    position_size: quantity,
                    stop_loss,
                    take_profit,
                    client_order_id,
                    sent_at: tokio::time::Instant::now(),
                });
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to send open position order");
                let _ = self.auto_event_tx.send(AutoEvent::BotError {
                    bot_id: self.bot.id,
                    error: format!("Failed to send open {} order: {}", side, e),
                });
            }
        }
    }

    pub(crate) async fn close_position(&mut self, reason: &str) {
        if !self.has_position() {
            return;
        }

        let side = self.bot.current_side.clone().unwrap_or_default();
        let (order_side, position_side) = match side.as_str() {
            "long" => (OrderSide::Sell, Some(PositionSide::Long)),
            "short" => (OrderSide::Buy, Some(PositionSide::Short)),
            _ => {
                warn!(bot_id = %self.bot.id, side = %side, "Unknown position side, cannot close");
                return;
            }
        };

        let client_order_id = format!("auto:close:{}:{}", reason, self.bot.id);

        let result = self.order_executor.send_command(OrderCommand::PlaceOrder {
            symbol: self.bot.symbol.clone(),
            side: order_side,
            amount: self.bot.position_size,
            price: None,
            reduce_only: true,
            position_side,
            client_order_id: Some(client_order_id.clone()),
        }).await;

        match result {
            Ok(()) => {
                info!(
                    bot_id = %self.bot.id, side = %side,
                    entry_price = self.bot.entry_price,
                    close_price = self.current_price,
                    reason = %reason,
                    "Position closing order sent, awaiting confirmation"
                );

                self.pending_close = Some(PendingClose {
                    side: side.clone(),
                    reason: reason.to_string(),
                    entry_price: self.bot.entry_price,
                    position_size: self.bot.position_size,
                    unrealized_pnl: self.bot.unrealized_pnl,
                    client_order_id,
                    sent_at: tokio::time::Instant::now(),
                });
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to send close position order");
                let _ = self.auto_event_tx.send(AutoEvent::BotError {
                    bot_id: self.bot.id,
                    error: format!("Failed to send close order ({}): {}", reason, e),
                });
            }
        }
    }

    pub(crate) async fn apply_pending_open(&mut self, fill_price: f64, filled_qty: f64) {
        let pending = match self.pending_open.take() {
            Some(p) => p,
            None => return,
        };

        let actual_qty = if filled_qty > 0.0 { filled_qty } else { pending.position_size };

        let price_deviation = if pending.entry_price > 0.0 {
            (fill_price - pending.entry_price).abs() / pending.entry_price
        } else {
            0.0
        };

        let (stop_loss, take_profit) = if price_deviation > 0.005 {
            let snapshot = self.market_data_provider.get_market_snapshot(
                &self.bot.exchange,
                &self.bot.symbol,
                self.bot.market_type.as_str(),
            ).await;
            let atr = if snapshot.indicators.atr > 0.0 { snapshot.indicators.atr } else { fill_price * 0.02 };
            let sl = strategy::compute_stop_loss(fill_price, &pending.side, atr);
            let tp = strategy::compute_take_profit(fill_price, &pending.side, atr);
            warn!(
                bot_id = %self.bot.id,
                requested_price = pending.entry_price, fill_price,
                deviation_pct = format!("{:.2}%", price_deviation * 100.0),
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

        self.bot.current_side = Some(pending.side.clone());
        self.bot.entry_price = fill_price;
        self.bot.position_size = actual_qty;
        self.bot.stop_loss = stop_loss;
        self.bot.take_profit = take_profit;
        self.bot.unrealized_pnl = 0.0;
        self.position_opened_at = Some(tokio::time::Instant::now());

        self.save_position().await;

        let trade_type = match pending.side.as_str() {
            "long" => "open_long",
            "short" => "open_short",
            _ => "open",
        };
        let trade_side = match pending.side.as_str() {
            "long" => "buy",
            "short" => "sell",
            _ => "buy",
        };
        let _ = self.store.record_trade(
            self.bot.id,
            self.bot.user_id,
            &self.bot.symbol,
            &self.bot.exchange,
            trade_side,
            trade_type,
            "llm",
            fill_price,
            actual_qty,
            0.0,
            0.0,
            None,
        ).await;

        let _ = self.auto_event_tx.send(AutoEvent::PositionOpened {
            bot_id: self.bot.id,
            side: pending.side.clone(),
            price: fill_price,
            quantity: actual_qty,
        });

        if actual_qty < pending.position_size {
            warn!(
                bot_id = %self.bot.id,
                requested = pending.position_size,
                filled = actual_qty,
                "Partial open: position opened with less than requested quantity"
            );
        }
    }

    pub(crate) async fn apply_pending_close(&mut self, fill_price: f64, filled_qty: f64) {
        let pending = match self.pending_close.take() {
            Some(p) => p,
            None => return,
        };

        let actual_qty = if filled_qty > 0.0 { filled_qty } else { pending.position_size };

        let realized_pnl = match pending.side.as_str() {
            "long" => (fill_price - pending.entry_price) * actual_qty,
            "short" => (pending.entry_price - fill_price) * actual_qty,
            _ => 0.0,
        };
        let pnl_pct = if pending.entry_price > 0.0 {
            match pending.side.as_str() {
                "long" => (fill_price - pending.entry_price) / pending.entry_price * 100.0,
                "short" => (pending.entry_price - fill_price) / pending.entry_price * 100.0,
                _ => 0.0,
            }
        } else {
            0.0
        };

        info!(
            bot_id = %self.bot.id, side = %pending.side,
            entry_price = pending.entry_price,
            close_price = fill_price,
            requested_qty = pending.position_size,
            actual_qty,
            pnl = realized_pnl,
            pnl_pct = pnl_pct,
            reason = %pending.reason,
            "Close order confirmed, position closed"
        );

        let effective_trade_type = match pending.reason.as_str() {
            "stop_loss" => "stop_loss",
            "take_profit" => "take_profit",
            _ => match pending.side.as_str() {
                "long" => "close_long",
                "short" => "close_short",
                _ => "close",
            },
        };

        let trigger_source = match pending.reason.as_str() {
            "stop_loss" | "take_profit" | "position_timeout" => "risk_control",
            _ => "llm",
        };

        let trade_side = match pending.side.as_str() {
            "long" => "sell",
            "short" => "buy",
            _ => "sell",
        };
        let _ = self.store.record_trade(
            self.bot.id,
            self.bot.user_id,
            &self.bot.symbol,
            &self.bot.exchange,
            trade_side,
            effective_trade_type,
            trigger_source,
            fill_price,
            actual_qty,
            realized_pnl,
            pnl_pct,
            None,
        ).await;

        self.bot.total_pnl += realized_pnl;
        self.bot.total_trades += 1;
        if realized_pnl >= 0.0 {
            self.bot.win_trades += 1;
            self.consecutive_losses = 0;
        } else {
            self.bot.loss_trades += 1;
            self.consecutive_losses += 1;
        }

        if actual_qty < pending.position_size {
            warn!(
                bot_id = %self.bot.id,
                requested = pending.position_size,
                filled = actual_qty,
                remaining = self.bot.position_size,
                stop_loss = self.bot.stop_loss,
                take_profit = self.bot.take_profit,
                "Partial close: remaining position still open, SL/TP price levels remain valid"
            );
            self.bot.position_size -= actual_qty;
            self.save_position().await;
            self.save_stats().await;
            return;
        }

        self.bot.current_side = Some("none".to_string());
        self.bot.entry_price = 0.0;
        self.bot.position_size = 0.0;
        self.bot.stop_loss = 0.0;
        self.bot.take_profit = 0.0;
        self.bot.unrealized_pnl = 0.0;
        self.position_opened_at = None;

        self.save_position().await;
        self.save_stats().await;

        let _ = self.auto_event_tx.send(AutoEvent::PositionClosed {
            bot_id: self.bot.id,
            side: pending.side.clone(),
            price: fill_price,
            pnl: realized_pnl,
        });
    }

    pub(crate) fn rollback_pending_open(&mut self) {
        if let Some(pending) = self.pending_open.take() {
            warn!(
                bot_id = %self.bot.id, side = %pending.side,
                "Open order failed, position NOT opened"
            );
            let _ = self.auto_event_tx.send(AutoEvent::BotError {
                bot_id: self.bot.id,
                error: format!("Open {} order failed, position not opened", pending.side),
            });
        }
    }

    pub(crate) fn rollback_pending_close(&mut self) {
        if let Some(pending) = self.pending_close.take() {
            warn!(
                bot_id = %self.bot.id, side = %pending.side,
                "Close order failed, position still open"
            );
            let _ = self.auto_event_tx.send(AutoEvent::BotError {
                bot_id: self.bot.id,
                error: format!("Close {} order failed, position still open", pending.side),
            });
        }
    }
}
