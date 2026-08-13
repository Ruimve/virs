

use tracing::{error, warn};
use uuid::Uuid;

use crate::chat::ai::{BotAction, BotDecision};
use crate::chat::ports::BotMarketSnapshot;
use crate::chat::strategy;
use virs_prompt::{render, RenderContext};
use virs_type::{PositionSide, StrategyType};

use crate::chat::worker::BotWorker;
use super::format_close_event;

impl BotWorker {
    pub(crate) async fn on_llm_decision(&mut self) {
        /* LLM决策流程：有pending订单时跳过 -> 刷新持仓 -> 检查AI可用性 ->
         * 构建Prompt -> 调用LLM -> 保存分析日志 -> 执行决策(开仓/平仓/观望) */
        if self.is_pending() {
            warn!(bot_id = %self.bot.id, "Pending order in progress, skipping LLM decision");
            return;
        }

        self.refresh_position_from_pe().await;

        if !self
            .ai_service
            .is_available_for_user(self.bot.user_id)
            .await
        {
            warn!(bot_id = %self.bot.id, "AI service not available, skipping decision");
            return;
        }

        let (system_prompt, user_prompt) = match self.build_llm_prompt().await {
            Some(prompts) => prompts,
            None => return,
        };

        let decision_result = self
            .ai_service
            .chat_decision(self.bot.user_id, &system_prompt, &user_prompt)
            .await;
        let (decision, raw_llm_response, llm_model) = match decision_result {
            Some((d, raw, m)) => (Some(d), Some(raw), m),
            None => (None, None, String::new()),
        };

        let (action, log_id) = self
            .handle_llm_result(
                &decision,
                &system_prompt,
                &user_prompt,
                raw_llm_response.as_ref(),
                &llm_model,
            )
            .await;


        match action {
            BotAction::OpenLong => self.long.log_id = log_id,
            BotAction::OpenShort => self.short.log_id = log_id,
            BotAction::ClosePosition => {


                if self.has_position_side(PositionSide::Long)
                    || !self.has_position_side(PositionSide::Short)
                {
                    self.long.log_id = log_id;
                } else {
                    self.short.log_id = log_id;
                }
            }
            BotAction::Hold => {

                self.long.log_id = log_id;
            }
        }

        let intercept_reason = self.execute_decision(&action, decision.as_ref()).await;
        if let Some(reason) = intercept_reason {
            warn!(bot_id = %self.bot.id, action = %action.as_str(), intercept_reason = %reason, "Decision intercepted");
            let exec_status = match action {
                BotAction::OpenLong | BotAction::OpenShort => "open_failed",
                BotAction::ClosePosition => "close_failed",
                BotAction::Hold => "hold",
            };

            let log_ids: Vec<Uuid> = [self.long.log_id.take(), self.short.log_id.take()]
                .into_iter()
                .flatten()
                .collect();
            for log_id in log_ids {
                if let Err(e) = self
                    .store
                    .update_analysis_log_execution(log_id, exec_status, Some(&reason))
                    .await
                {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to update intercept log");
                }
            }
        } else if matches!(action, BotAction::Hold) {
            if let Some(log_id) = self.long.log_id.take() {
                if let Err(e) = self
                    .store
                    .update_analysis_log_execution(log_id, "hold", None)
                    .await
                {
                    error!(bot_id = %self.bot.id, error = %e, "Failed to update hold log");
                }
            }
            self.short.log_id = None;
        }

        if !matches!(action, BotAction::Hold) {
            if let Err(e) = self.store.update_last_decided(self.bot.id).await {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to update last decided");
            }
        }
    }

    async fn build_llm_prompt(&self) -> Option<(String, String)> {
        let snapshot = match self
            .market_data_provider
            .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
            .await
        {
            Ok(s) => match BotMarketSnapshot::from_base(s) {
                Ok(snap) => snap,
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to parse indicators for LLM prompt");
                    return None;
                }
            },
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch market snapshot for LLM prompt");
                return None;
            }
        };

        if snapshot.base.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping decision");
            return None;
        }

        let account = match self
            .market_data_provider
            .get_account_balance(&self.bot.exchange)
            .await
        {
            Ok(b) => b,
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch account balance for LLM prompt");
                return None;
            }
        };
        let margin_usage_rate = if account.total > 0.0 {
            account.used / account.total
        } else {
            0.0
        };


        let position_info = {
            let long_info = match &self.long.position {
                Some(p) if p.is_open() => strategy::format_position_info(
                    p,
                    Some("long"),
                    snapshot.base.current_price,
                ),
                _ => "无仓位".to_string(),
            };
            let short_info = match &self.short.position {
                Some(p) if p.is_open() => strategy::format_position_info(
                    p,
                    Some("short"),
                    snapshot.base.current_price,
                ),
                _ => "无仓位".to_string(),
            };
            format!("多：\n{}\n空：\n{}", long_info, short_info)
        };


        let stop_take_profit_info = {
            let has_long = self.has_position_side(PositionSide::Long);
            let has_short = self.has_position_side(PositionSide::Short);
            let long_info =
                strategy::format_stop_take_profit(self.long.stop_loss, self.long.take_profit);
            let short_info =
                strategy::format_stop_take_profit(self.short.stop_loss, self.short.take_profit);
            if has_long && has_short {
                if long_info.is_empty() || long_info == "未设置" {
                    if short_info.is_empty() || short_info == "未设置" {
                        "未设置".to_string()
                    } else {
                        format!("空：\n{}", short_info)
                    }
                } else if short_info.is_empty() || short_info == "未设置" {
                    format!("多：\n{}", long_info)
                } else {
                    format!("多：\n{}\n空：\n{}", long_info, short_info)
                }
            } else if has_long {
                long_info
            } else if has_short {
                short_info
            } else {
                "未设置".to_string()
            }
        };

        let position_duration = if self.has_any_position() {
            let mut parts: Vec<String> = Vec::new();
            if self.has_position_side(PositionSide::Long) {
                if let Some(opened_at) = self.long.position_opened_at {
                    let elapsed = opened_at.elapsed();
                    let hours = elapsed.as_secs() / 3600;
                    let mins = (elapsed.as_secs() % 3600) / 60;
                    parts.push(format!("多：{}小时{}分钟", hours, mins));
                } else {
                    parts.push("多：未知".to_string());
                }
            }
            if self.has_position_side(PositionSide::Short) {
                if let Some(opened_at) = self.short.position_opened_at {
                    let elapsed = opened_at.elapsed();
                    let hours = elapsed.as_secs() / 3600;
                    let mins = (elapsed.as_secs() % 3600) / 60;
                    parts.push(format!("空：{}小时{}分钟", hours, mins));
                } else {
                    parts.push("空：未知".to_string());
                }
            }
            parts.join("，")
        } else {
            "无持仓".to_string()
        };

        let recent_close_info = {
            let mut parts: Vec<String> = Vec::new();
            if let Some((side, reason, closed_at)) = &self.long.last_close_event {
                parts.push(format_close_event(side, reason, closed_at));
            }
            if let Some((side, reason, closed_at)) = &self.short.last_close_event {
                parts.push(format_close_event(side, reason, closed_at));
            }
            if parts.is_empty() {
                "无".to_string()
            } else {
                parts.join("\n")
            }
        };

        let ctx = RenderContext {
            timestamp: chrono::Utc::now()
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
            symbol: self.bot.symbol.clone(),
            exchange: self.bot.exchange.clone(),
            total_balance: account.total,
            available_balance: account.free,
            used_margin: account.used,
            margin_usage_rate: margin_usage_rate * 100.0,
            leverage: self.bot.leverage,
            position_info,
            position_duration,
            stop_take_profit_info,
            recent_close_info,
            funding_rate: snapshot.base.funding_rate,
            funding_next_time: snapshot.base.funding_next_time,
            total_trades: self.bot.total_trades,
            win_trades: self.bot.win_trades,
            loss_trades: self.bot.loss_trades,
            total_pnl: self.bot.total_pnl,
            consecutive_losses: self.consecutive_losses,
            trigger_reason: "scheduled".to_string(),
            min_qty: snapshot.base.min_qty,
            ind: snapshot.indicators,

            event_flag: false,
            event_description: String::new(),
        };


        /* strategy_file必须在bot创建时绑定，不能为NULL，否则无法构建Prompt */
        let (system_prompt, user_prompt) = {
            let file_name = match self.bot.strategy_file.as_deref() {
                Some(f) => f,
                None => {
                    error!(
                        bot_id = %self.bot.id,
                        "Bot has no strategy_file bound — cannot build LLM prompt. \
                         Stopping decision cycle."
                    );
                    return None;
                }
            };

            match self
                .prompt_loader
                .get_prompt(StrategyType::Chat, file_name)
                .await
            {
                Some(tpl) => {
                    let user = render(&tpl.user_prompt_template, &ctx);
                    let base_system = self
                        .bot
                        .system_prompt
                        .as_deref()
                        .unwrap_or(&tpl.system_prompt);
                    /* 自动拼接共享的JSON输出格式约束，确保所有策略使用统一的输出格式 */
                    let system = match self.prompt_loader.get_output_format().await {
                        Some(fmt) => format!("{base_system}\n\n{fmt}"),
                        None => {
                            warn!(
                                bot_id = %self.bot.id,
                                "Output format not loaded from strategies/meta.json — using system_prompt without JSON format constraint"
                            );
                            base_system.to_string()
                        }
                    };
                    (system, user)
                }
                None => {
                    error!(
                        bot_id = %self.bot.id,
                        strategy_file = file_name,
                        "Strategy file not found in loader — cannot build LLM prompt. \
                         Stopping decision cycle. Check STRATEGIES_DIR and strategy files."
                    );
                    return None;
                }
            }
        };

        Some((system_prompt, user_prompt))
    }

    async fn handle_llm_result(
        &mut self,
        decision: &Option<BotDecision>,
        system_prompt: &str,
        user_prompt: &str,
        raw_llm_response: Option<&serde_json::Value>,
        llm_model: &str,
    ) -> (BotAction, Option<Uuid>) {
        match decision {
            Some(d) => {
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
                if let Some(raw_llm_response) = raw_llm_response {
                    if let Some(obj) = result.as_object_mut() {
                        obj.insert("raw_llm_response".to_string(), raw_llm_response.clone());
                    } else {
                        error!(
                            bot_id = %self.bot.id,
                            "LLM result is not a JSON object — cannot insert raw_llm_response"
                        );
                    }
                }
                let log_id = match self
                    .store
                    .save_analysis_log(
                        self.bot.id,
                        "periodic",
                        system_prompt,
                        user_prompt,
                        &result,
                        None,
                        llm_model,
                        &self.bot.strategy_file,
                    )
                    .await
                {
                    Ok(id) => Some(id),
                    Err(e) => {
                        error!(bot_id = %self.bot.id, error = %e, "Failed to save analysis log");
                        None
                    }
                };

                (d.action.clone(), log_id)
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
                let log_id = match self
                    .store
                    .save_analysis_log(
                        self.bot.id,
                        "periodic",
                        system_prompt,
                        user_prompt,
                        &result,
                        Some("LLM call failed"),
                        llm_model,
                        &self.bot.strategy_file,
                    )
                    .await
                {
                    Ok(id) => Some(id),
                    Err(e) => {
                        error!(bot_id = %self.bot.id, error = %e, "Failed to save analysis log");
                        None
                    }
                };

                (BotAction::Hold, log_id)
            }
        }
    }

    pub(crate) async fn execute_decision(
        &mut self,
        action: &BotAction,
        decision: Option<&BotDecision>,
    ) -> Option<String> {
        /* 执行LLM决策：开仓需置信度>=0.6且无同方向持仓且不在冷却期；平仓需有持仓 */
        if matches!(action, BotAction::Hold) {
            return None;
        }

        if self.is_pending() {
            warn!(bot_id = %self.bot.id, "Pending order in progress, skipping decision execution");
            return Some("有待确认订单，跳过本次决策".to_string());
        }

        /* 开仓置信度阈值0.6：低于此值降级为Hold，避免低质量信号导致亏损 */
        if matches!(action, BotAction::OpenLong | BotAction::OpenShort) {
            if let Some(d) = decision {
                if d.confidence < 0.6 {
                    warn!(
                        bot_id = %self.bot.id,
                        action = %action.as_str(),
                        confidence = d.confidence,
                        "Confidence below 0.6 threshold for opening position, downgrading to Hold"
                    );
                    return Some(format!(
                        "置信度 {:.2} 低于 0.6 阈值，降级为观望",
                        d.confidence
                    ));
                }
            }
        }

        let snapshot = match self
            .market_data_provider
            .get_market_snapshot(&self.bot.exchange, &self.bot.symbol)
            .await
        {
            Ok(s) => match BotMarketSnapshot::from_base(s) {
                Ok(snap) => snap,
                Err(e) => {
                    warn!(bot_id = %self.bot.id, error = %e, "Failed to parse indicators for decision execution");
                    return Some(format!("指标解析失败，跳过决策: {}", e));
                }
            },
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Failed to fetch market snapshot for decision execution");
                return Some(format!("市场快照获取失败，跳过决策: {}", e));
            }
        };

        if snapshot.base.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping decision execution");
            return Some("市场快照价格为 0，跳过决策".to_string());
        }

        match action {
            BotAction::OpenLong | BotAction::OpenShort => {
                if let Some(d) = decision {
                    self.apply_non_structural_params(d).await;
                }

                let side = match action {
                    BotAction::OpenLong => "long",
                    BotAction::OpenShort => "short",
                    _ => unreachable!(),
                };
                let position_side = match side {
                    "long" => PositionSide::Long,
                    "short" => PositionSide::Short,
                    _ => unreachable!(),
                };


                if self.has_position_side(position_side.clone()) {
                    warn!(bot_id = %self.bot.id, side = %side, "Already has position on this side, cannot open");
                    return Some("该方向已有仓位".to_string());
                }

                if let Some(remaining) = self.cooldown_remaining_secs(side) {
                    let last_event = self.side(&position_side).last_close_event.as_ref();
                    let (closed_side, close_reason, closed_at) = match last_event {
                        Some(ev) => ev,
                        None => {
                            error!(
                                bot_id = %self.bot.id,
                                side = %side,
                                remaining_secs = remaining,
                                "In cooldown but last_close_event is None — data inconsistency, skipping open"
                            );
                            return Some("冷却期中但关闭事件丢失，无法开仓".to_string());
                        }
                    };
                    warn!(
                        bot_id = %self.bot.id,
                        new_side = %side,
                        last_close_side = %closed_side,
                        last_close_reason = %close_reason,
                        last_close_at = %closed_at,
                        remaining_secs = remaining,
                        "In cooldown period, skipping open"
                    );
                    let reason_cn = match close_reason.as_str() {
                        "stop_loss" => "止损",
                        "take_profit" => "止盈",
                        "position_timeout" => "持仓超时",
                        "llm_decision" => "LLM平仓",
                        _ => "其他",
                    };
                    let side_cn = if side == "long" { "多" } else { "空" };
                    let closed_side_cn = if closed_side == "long" { "多" } else { "空" };
                    return Some(format!(
                        "冷却期剩 {}s，不允许开{}（上次：{}{}）",
                        remaining, side_cn, closed_side_cn, reason_cn
                    ));
                }
                self.open_position(side, &snapshot).await;

                let pending_set = self.side(&position_side).pending_open.is_some();
                if !pending_set {
                    return Some("开仓订单发送失败".to_string());
                }
                None
            }
            BotAction::ClosePosition => {
                if !self.has_any_position() {
                    warn!(bot_id = %self.bot.id, "No position to close");
                    return Some("无仓位可平".to_string());
                }


                let mut any_pending = false;
                if self.has_position_side(PositionSide::Long) {
                    self.close_position(PositionSide::Long, "llm_decision").await;
                    if self.long.pending_close.is_some() {
                        any_pending = true;
                    }
                }
                if self.has_position_side(PositionSide::Short) {
                    self.close_position(PositionSide::Short, "llm_decision").await;
                    if self.short.pending_close.is_some() {
                        any_pending = true;
                    }
                }

                if !any_pending {
                    return Some("平仓订单发送失败".to_string());
                }
                None
            }
            BotAction::Hold => {
                warn!(bot_id = %self.bot.id, "Hold action reached execute_action, skipping");
                None
            }
        }
    }

    async fn apply_non_structural_params(&mut self, d: &BotDecision) {
        if let Some(ref regime) = d.market_regime {
            self.bot.market_regime = Some(regime.clone());
        }

        let regime = match self.bot.market_regime.as_deref() {
            Some(r) => r,
            None => {
                warn!(bot_id = %self.bot.id, "market_regime is None — skipping AI analysis update to avoid default value");
                return;
            }
        };

        let analysis = match d.analysis.as_deref() {
            Some(a) => a,
            None => {
                warn!(bot_id = %self.bot.id, "analysis is None — skipping AI analysis update to avoid overwriting historical value");
                return;
            }
        };

        if let Err(e) = self
            .store
            .update_ai_analysis(
                self.bot.id,
                regime,
                self.bot.leverage,
                analysis,
            )
            .await
        {
            warn!(bot_id = %self.bot.id, error = %e, "Failed to update AI analysis");
        }
    }
}
