use std::collections::HashSet;

use tracing::{debug, info, warn};

use crate::bot::semi_automatic_grid::ai::{GridAction, GridDecision};
use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::{self, GridEvent};
use crate::bot::semi_automatic_grid::utils::prompt::{self, PromptContext};
use crate::bot::semi_automatic_grid::worker::GridWorker;

/** LLM 运行时系统提示词

定义 AI 助手的角色、决策规则和注意事项 */
impl GridWorker {
/** LLM 周期性决策入口

获取市场快照、构建 prompt、调用 AI 服务、执行决策 */
    pub(crate) async fn on_llm_decision(&mut self) {
        info!(bot_id = %self.bot.id, "LLM decision tick");

        if !self.ai_service.is_available_for_user(&self.bot.user_id).await {
            warn!(bot_id = %self.bot.id, "AI service not available, skipping LLM decision");
            let _ = self.grid_event_tx.send(GridEvent::BotError {
                bot_id: self.bot.id,
                error: "LLM decision skipped: AI service not configured".to_string(),
            });
            return;
        }

        let is_initial = self.bot.upper_price <= 0.0 || self.bot.lower_price <= 0.0 || self.levels.is_empty();

        let (system_prompt, user_prompt) = match self.build_llm_prompt().await {
            Some(prompts) => prompts,
            None => return,
        };

        let decision_result = self.ai_service.grid_decision(&self.bot.user_id, &system_prompt, &user_prompt).await;
        let (decision, raw_llm_response) = match decision_result {
            Some((d, raw)) => (Some(d), Some(raw)),
            None => (None, None),
        };
        let action = self.handle_llm_result(&decision, &system_prompt, &user_prompt, raw_llm_response.as_ref(), is_initial).await;

        self.execute_decision(&action, decision.as_ref()).await;
        let _ = self.store.update_last_adjusted(self.bot.id).await;
    }

/** 构建 LLM 调用所需的 system_prompt 和 user_prompt

获取市场快照和账户余额，渲染 prompt 模板。
当市场数据不可用时返回 None */
    async fn build_llm_prompt(&self) -> Option<(String, String)> {
        let snapshot = self.market_data_provider.get_market_snapshot(&self.bot.exchange, &self.bot.symbol).await;

        if snapshot.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "Market snapshot has zero price, skipping LLM decision");
            let _ = self.grid_event_tx.send(GridEvent::BotError {
                bot_id: self.bot.id,
                error: "LLM decision skipped: market data unavailable".to_string(),
            });
            return None;
        }

        let grid_status = if self.paused { "paused" } else if self.levels.is_empty() { "empty" } else { "running" };

        let current_price = self.current_price;
        let long_qty: f64 = self.levels.iter().filter(|l| l.hold_quantity > 0.0).map(|l| l.hold_quantity).sum();
        let short_qty: f64 = self.levels.iter().filter(|l| l.hold_quantity < 0.0).map(|l| l.hold_quantity.abs()).sum();
        let long_cost: f64 = self.levels.iter().filter(|l| l.hold_quantity > 0.0 && l.avg_buy_price > 0.0).map(|l| l.avg_buy_price * l.hold_quantity).sum();
        let short_cost: f64 = self.levels.iter().filter(|l| l.hold_quantity < 0.0 && l.avg_buy_price > 0.0).map(|l| l.avg_buy_price * l.hold_quantity.abs()).sum();
        let long_avg = if long_qty > 0.0 { long_cost / long_qty } else { 0.0 };
        let short_avg = if short_qty > 0.0 { short_cost / short_qty } else { 0.0 };
        let long_pnl = if long_qty > 0.0 && current_price > 0.0 { (current_price - long_avg) * long_qty } else { 0.0 };
        let short_pnl = if short_qty > 0.0 && current_price > 0.0 { (short_avg - current_price) * short_qty } else { 0.0 };

        let position_info = if long_qty <= 0.0 && short_qty <= 0.0 {
            "none".to_string()
        } else {
            let mut s = String::new();
            if long_qty > 0.0 {
                s.push_str(&format!("- Long: 币数 {:.6}, 均价 {:.2}, 未实现盈亏 {:.2} USDT", long_qty, long_avg, long_pnl));
            }
            if short_qty > 0.0 {
                if !s.is_empty() { s.push('\n'); }
                s.push_str(&format!("- Short: 币数 {:.6}, 均价 {:.2}, 未实现盈亏 {:.2} USDT", short_qty, short_avg, short_pnl));
            }
            s
        };

        let current_grid_config = prompt::format_grid_config(
            grid_status,
            self.bot.upper_price,
            self.bot.lower_price,
            self.bot.grid_count,
            self.bot.grid_profit_pct,
            self.bot.quantity_per_grid,
            &self.levels,
        );

        let account = self.market_data_provider.get_account_balance(&self.bot.exchange).await;
        let margin_usage_rate = if account.total > 0.0 { account.used / account.total } else { 0.0 };

        let template = super::super::types::DEFAULT_USER_PROMPT_TEMPLATE;
        let ctx = PromptContext {
            timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            symbol: self.bot.symbol.clone(),
            total_balance: account.total,
            available_balance: account.free,
            used_margin: account.used,
            margin_usage_rate,
            leverage: self.bot.leverage,
            grid_status: grid_status.to_string(),
            last_adjust_time: self.bot.last_adjusted_at
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            consecutive_losses: self.consecutive_losses,
            current_grid_config,
            position_info,
            funding_rate: snapshot.indicators.funding_rate,
            funding_next_time: "N/A".to_string(),
            event_flag: false,
            event_description: String::new(),
            trigger_reason: "scheduled_15m".to_string(),
            ind: snapshot.indicators,
        };

        let user_prompt = prompt::render_prompt(template, &ctx);
        let system_prompt = self.bot.system_prompt.as_deref().unwrap_or(types::DEFAULT_SYSTEM_PROMPT).to_string();

        Some((system_prompt, user_prompt))
    }

/** 处理 LLM 调用结果，记录分析日志，返回最终 action

LLM 成功时记录结果并返回其 action，失败时回退到规则决策 */
    async fn handle_llm_result(
        &mut self,
        decision: &Option<GridDecision>,
        system_prompt: &str,
        user_prompt: &str,
        raw_llm_response: Option<&serde_json::Value>,
        is_initial: bool,
    ) -> GridAction {
        match decision {
            Some(d) => {
                info!(bot_id = %self.bot.id, action = d.action.as_str(), reason = %d.reason, confidence = d.confidence, source = "llm", "LLM decision");

                if let Some(ref w) = d.funding_rate_warning {
                    warn!(bot_id = %self.bot.id, warning = %w, "Funding rate warning from LLM");
                }
                if let Some(ref w) = d.risk_warning {
                    warn!(bot_id = %self.bot.id, warning = %w, "Risk warning from LLM");
                }
                if let Some(ref w) = d.event_impact {
                    info!(bot_id = %self.bot.id, impact = %w, "Event impact from LLM");
                }

                let mut result = serde_json::json!({
                    "action": d.action.as_str(),
                    "reason": d.reason,
                    "confidence": d.confidence,
                    "upper_price": d.upper_price,
                    "lower_price": d.lower_price,
                    "cancel_level": d.cancel_level,
                    "cancel_side": d.cancel_side,
                    "grid_count": d.grid_count,
                    "grid_profit_pct": d.grid_profit_pct,
                    "quantity_per_grid": d.quantity_per_grid,
                    "leverage": d.leverage,
                    "market_regime": d.market_regime,
                    "funding_rate_warning": d.funding_rate_warning,
                    "event_impact": d.event_impact,
                    "risk_warning": d.risk_warning,
                    "analysis": d.analysis,
                });
                if let Some(raw) = raw_llm_response {
                    result.as_object_mut().unwrap().insert("raw_llm_response".to_string(), raw.clone());
                }
                let _ = self.store.save_analysis_log(
                    self.bot.id, if is_initial { "initial" } else { "periodic" }, system_prompt, user_prompt,
                    &result, None,
                ).await;

                d.action.clone()
            }
            None => {
                let rule_action = self.simple_rule_decision();
                warn!(bot_id = %self.bot.id, action = rule_action.as_str(), source = "rule_fallback", "LLM call failed, falling back to rule-based decision");

                let result = serde_json::json!({
                    "action": rule_action.as_str(),
                    "reason": "LLM call failed, using rule-based fallback",
                });
                let _ = self.store.save_analysis_log(
                    self.bot.id, if is_initial { "initial" } else { "periodic" }, system_prompt, user_prompt,
                    &result, Some("LLM call failed"),
                ).await;

                let _ = self.grid_event_tx.send(GridEvent::BotError {
                    bot_id: self.bot.id,
                    error: "LLM call failed, using rule-based fallback".to_string(),
                });
                rule_action
            }
        }
    }

/** 执行 AI 决策

参数应用优先级：无论 action 是什么，LLM 返回的非结构参数（market_regime、leverage、
quantity_per_grid）始终先应用到 bot 配置和 DB。
结构参数（upper_price、lower_price、grid_count、grid_profit_pct）仅在
action 为 AdjustGrid 或首次初始化时才应用，避免 Hold/PauseGrid 等操作
意外触发网格重建。

层级方向（side）由 current_price 自动判定，不再依赖 LLM 返回的 grid_levels_json。 */
    pub(crate) async fn execute_decision(&mut self, action: &GridAction, decision: Option<&GridDecision>) {
        let needs_params = self.bot.upper_price <= 0.0 || self.bot.lower_price <= 0.0;
        let allow_structure_change = needs_params || matches!(action, GridAction::AdjustGrid { .. });

        let mut structure_changed = false;
        if let Some(d) = decision {
            structure_changed = self.apply_llm_params(d, needs_params, allow_structure_change).await;
        }

        match action {
            GridAction::PauseGrid => {
                self.pause_with_cancel("LLM decision").await;
            }
            GridAction::RunGrid => {
                if self.paused {
                    self.paused = false;
                    self.save_stats().await;
                    self.place_initial_orders().await;
                    info!(bot_id = %self.bot.id, "Grid resumed by decision");
                }
            }
            GridAction::ReducePosition => {
                let min_qty = 1.0;
                if decision.map_or(true, |d| d.quantity_per_grid.is_none()) {
                    let new_qty = (self.bot.quantity_per_grid * 0.5).max(min_qty);
                    self.bot.quantity_per_grid = new_qty;
                    let _ = self.store.update_quantity_per_grid(self.bot.id, new_qty).await;
                }
                let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
                    symbol: Some(self.bot.symbol.clone()),
                }).await;
                self.recalculate_levels();
                self.save_stats().await;
                if !self.paused {
                    self.place_initial_orders().await;
                }
                warn!(bot_id = %self.bot.id, quantity_per_grid = self.bot.quantity_per_grid, "Position reduced by decision");
            }
            GridAction::AdjustGrid { .. } => {
                if needs_params {
                    if !self.levels.is_empty() && !self.paused {
                        self.place_initial_orders().await;
                    }
                } else if let Some(d) = decision {
                    self.adjust_grid(d.upper_price, d.lower_price, structure_changed).await;
                }
            }
            GridAction::CancelOrder { level, side } => {
                let level_idx = self.levels.iter().position(|l| l.level == *level);
                if let Some(idx) = level_idx {
                    self.cancel_level_order(idx, side).await;
                    info!(bot_id = %self.bot.id, level = level, side = %side, "Order canceled by LLM decision");
                } else {
                    warn!(bot_id = %self.bot.id, level = level, side = %side, "CancelOrder: level not found");
                }
            }
            GridAction::Hold => {
                if needs_params && !self.levels.is_empty() && !self.paused {
                    self.place_initial_orders().await;
                }
            }
        }
    }

/** 将 LLM 决策中的参数应用到 bot 配置并持久化

参数分两类：
- 非结构参数（始终应用）：market_regime、leverage、quantity_per_grid
- 结构参数（仅 allow_structure_change=true 时应用）：upper_price、lower_price、
  grid_count、grid_profit_pct，这些参数变化会触发 recalculate_levels

side 由 current_price 自动判定（低于当前价 buy，高于当前价 sell），
不再依赖 LLM 返回的 grid_levels_json。

无论 action 是什么，非结构参数始终先应用，确保 LLM 的市场判断和风控建议
（如 leverage 调整、market_regime 更新）不会因 action 类型而被跳过。 */
    async fn apply_llm_params(&mut self, d: &GridDecision, needs_params: bool, allow_structure_change: bool) -> bool {
        let mut structure_changed = false;

        if let Some(ref regime) = d.market_regime {
            self.bot.market_regime = Some(regime.clone());
        }
        if let Some(lev) = d.leverage {
            if lev > 0 {
                self.bot.leverage = lev;
            }
        }
        if let Some(qty) = d.quantity_per_grid {
            if qty > 0.0 {
                self.bot.quantity_per_grid = qty.max(1.0);
            }
        }

        if allow_structure_change {
            if let Some(count) = d.grid_count {
                if count > 0 && count != self.bot.grid_count {
                    self.bot.grid_count = count;
                    structure_changed = true;
                }
            }
            if let Some(pct) = d.grid_profit_pct {
                if pct > 0.0 && (pct - self.bot.grid_profit_pct).abs() > f64::EPSILON {
                    self.bot.grid_profit_pct = pct;
                    structure_changed = true;
                }
            }
            if needs_params {
                if let Some(upper) = d.upper_price {
                    if upper > 0.0 {
                        self.bot.upper_price = upper;
                        structure_changed = true;
                    }
                }
                if let Some(lower) = d.lower_price {
                    if lower > 0.0 {
                        self.bot.lower_price = lower;
                        structure_changed = true;
                    }
                }
                if self.bot.upper_price > 0.0 && self.bot.lower_price > 0.0 {
                    let _ = self.store.update_grid_params(self.bot.id, self.bot.upper_price, self.bot.lower_price).await;
                }
            }
        }

        if structure_changed {
            self.recalculate_levels();
        }

        let _ = self.store.update_ai_analysis(
            self.bot.id,
            self.bot.market_regime.as_deref().unwrap_or("ranging"),
            self.bot.upper_price,
            self.bot.lower_price,
            self.bot.grid_count,
            self.bot.grid_profit_pct,
            self.bot.quantity_per_grid,
            self.bot.leverage,
            d.analysis.as_deref().unwrap_or(""),
        ).await;

        self.save_stats().await;

        info!(
            bot_id = %self.bot.id,
            grid_count = self.bot.grid_count,
            grid_profit_pct = self.bot.grid_profit_pct,
            quantity_per_grid = self.bot.quantity_per_grid,
            leverage = self.bot.leverage,
            allow_structure_change,
            structure_changed,
            "LLM params applied"
        );

        structure_changed
    }

/** 简单规则决策（LLM 不可用时的回退策略）

价格突破网格上下界 2% 时暂停，价格回到区间内时恢复 */
    pub(crate) fn simple_rule_decision(&self) -> GridAction {
        if self.current_price > self.bot.upper_price * 1.02 {
            return GridAction::PauseGrid;
        }
        if self.current_price < self.bot.lower_price * 0.98 {
            return GridAction::PauseGrid;
        }
        if self.paused && self.current_price >= self.bot.lower_price && self.current_price <= self.bot.upper_price {
            return GridAction::RunGrid;
        }
        GridAction::Hold
    }

/** 处理外部调整信号

从数据库重新加载 bot 配置，检测参数变化并按需调整网格 */
    pub async fn on_adjust_signal(&mut self) {
        match self.store.load_bot(self.bot.id).await {
            Ok(Some(updated_bot)) => {
                let price_changed = (updated_bot.upper_price - self.bot.upper_price).abs() > f64::EPSILON
                    || (updated_bot.lower_price - self.bot.lower_price).abs() > f64::EPSILON;
                let structure_changed = updated_bot.grid_count != self.bot.grid_count
                    || (updated_bot.grid_profit_pct - self.bot.grid_profit_pct).abs() > f64::EPSILON;

                if price_changed || structure_changed {
                    if structure_changed {
                        self.bot.grid_count = updated_bot.grid_count;
                        self.bot.grid_profit_pct = updated_bot.grid_profit_pct;
                    }
                    let new_upper = if (updated_bot.upper_price - self.bot.upper_price).abs() > f64::EPSILON {
                        Some(updated_bot.upper_price)
                    } else {
                        None
                    };
                    let new_lower = if (updated_bot.lower_price - self.bot.lower_price).abs() > f64::EPSILON {
                        Some(updated_bot.lower_price)
                    } else {
                        None
                    };
                    self.adjust_grid(new_upper, new_lower, structure_changed).await;
                } else {
                    self.bot.quantity_per_grid = updated_bot.quantity_per_grid;
                    self.bot.dynamic_adjust = updated_bot.dynamic_adjust;
                    self.bot.adjust_interval_secs = updated_bot.adjust_interval_secs;
                }
                self.bot.system_prompt = updated_bot.system_prompt;
                self.bot.leverage = updated_bot.leverage;
                self.bot.market_regime = updated_bot.market_regime;
                self.bot.grid_levels_json = updated_bot.grid_levels_json;
                if !price_changed && !structure_changed {
                    info!(bot_id = %self.bot.id, "Adjust signal received, non-structural params updated");
                }
            }
            Ok(None) => {
                warn!(bot_id = %self.bot.id, "Adjust signal received but bot not found in store");
            }
            Err(e) => {
                warn!(bot_id = %self.bot.id, error = %e, "Adjust signal received but failed to load bot from store");
            }
        }
    }

/** 调整网格上下界并重建层级

参数:
- new_upper: 新的上界价格（None 表示不变）
- new_lower: 新的下界价格（None 表示不变）
- force_recalculate: 是否强制重建层级（结构参数变化时为 true） */
    pub async fn adjust_grid(&mut self, new_upper: Option<f64>, new_lower: Option<f64>, force_recalculate: bool) {
        let mut updated = false;
        if let Some(upper) = new_upper {
            if upper > 0.0 && upper != self.bot.upper_price {
                updated = true;
            }
        }
        if let Some(lower) = new_lower {
            if lower > 0.0 && lower != self.bot.lower_price {
                updated = true;
            }
        }

        if !updated && !force_recalculate {
            debug!(bot_id = %self.bot.id, "adjust_grid: no parameter changes");
            return;
        }

        let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
            symbol: Some(self.bot.symbol.clone()),
        }).await;

        if let Some(upper) = new_upper {
            if upper > 0.0 {
                self.bot.upper_price = upper;
            }
        }
        if let Some(lower) = new_lower {
            if lower > 0.0 {
                self.bot.lower_price = lower;
            }
        }

        if self.bot.upper_price <= self.bot.lower_price {
            warn!(
                bot_id = %self.bot.id,
                upper = self.bot.upper_price, lower = self.bot.lower_price,
                "adjust_grid: upper_price <= lower_price, skipping recalculate"
            );
            return;
        }

        let _ = self.store.update_grid_params(self.bot.id, self.bot.upper_price, self.bot.lower_price).await;
        self.recalculate_levels();
        self.save_stats().await;

        if !self.paused {
            self.place_initial_orders().await;
        }

        info!(
            bot_id = %self.bot.id, new_upper = self.bot.upper_price,
            new_lower = self.bot.lower_price, grid_count = self.levels.len(),
            "Grid adjusted"
        );

        let _ = self.grid_event_tx.send(GridEvent::GridAdjusted {
            bot_id: self.bot.id,
            upper_price: self.bot.upper_price,
            lower_price: self.bot.lower_price,
            level_count: self.levels.len(),
        });
    }

/** 重建网格层级，保留已有持仓

重新计算所有层级价格，将旧层级的持仓量、均价等状态
按价格匹配到新层级中 */
    pub fn recalculate_levels(&mut self) {
        let old_levels = std::mem::take(&mut self.levels);
        let holdings: Vec<crate::bot::semi_automatic_grid::types::GridLevel> = old_levels.iter()
            .filter(|l| l.hold_quantity.abs() > 0.0 || l.buy_filled || l.sell_filled)
            .cloned()
            .collect();
        drop(old_levels);

        self.levels = Self::calculate_levels(&self.bot, self.current_price);

        let max_dist = self.grid_spacing();

        let mut matched = HashSet::new();
        for old in &holdings {
            if let Some(idx) = self.find_level_by_price_within(old.price, max_dist) {
                if matched.contains(&idx) {
                    warn!(
                        bot_id = %self.bot.id, old_price = old.price, new_idx = idx,
                        "Multiple old holdings matched to same new level, skipping later one"
                    );
                    continue;
                }
                matched.insert(idx);
                let level = &mut self.levels[idx];
                let side_changed = old.side != level.side;
                if side_changed {
                    warn!(
                        bot_id = %self.bot.id, old_side = %old.side, new_side = %level.side,
                        old_price = old.price, new_level_price = level.price,
                        "Level side changed after grid adjustment, clearing holdings for this level"
                    );
                    level.buy_filled = false;
                    level.sell_filled = false;
                    level.last_fill_price = None;
                    level.trade_id = None;
                    level.buy_order_id = None;
                    level.sell_order_id = None;
                } else {
                    level.hold_quantity = old.hold_quantity;
                    level.avg_buy_price = old.avg_buy_price;
                    level.buy_filled = old.buy_filled;
                    level.sell_filled = old.sell_filled;
                    level.last_fill_price = old.last_fill_price;
                    level.trade_id = old.trade_id;
                }
            }
        }

        self.pending_orders.clear();
        info!(bot_id = %self.bot.id, grid_count = self.levels.len(), holdings_matched = holdings.len(), "Grid levels recalculated (preserved holdings by price)");
    }
}
