use std::collections::HashSet;

use tracing::{debug, info, warn};

use crate::bot::semi_automatic_grid::ai::{GridAction, GridDecision};
use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::GridEvent;
use crate::bot::semi_automatic_grid::utils::prompt::{self, PromptContext};
use crate::bot::semi_automatic_grid::worker::GridWorker;

/** LLM 运行时系统提示词

定义 AI 助手的角色、决策规则和注意事项 */
pub const LLM_RUNTIME_PROMPT: &str = r#"你是一位正在管理加密货币网格交易机器人的 AI 助手。你的职责是根据当前市场状态和机器人运行数据，做出最优决策。

## 决策规则
1. **run_grid**: 价格在网格区间内且市场状态适合网格交易时，继续运行
2. **pause_grid**: 价格突破网格区间（超过上下界 2%）、市场转为强趋势、或连续亏损时暂停
3. **adjust_grid**: 市场波动率显著变化，需要调整网格上下界时
4. **reduce_position**: 高波动或连续亏损时，减半仓位
5. **hold**: 当前状态良好，无需操作

## 注意
- 暂停后不会自动恢复，需要明确的 run_grid 指令
- adjust_grid 必须返回新的 upper_price 和 lower_price
- 优先保守操作，避免在不确定时频繁调整"#;

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

        let (system_prompt, user_prompt) = match self.build_llm_prompt().await {
            Some(prompts) => prompts,
            None => return,
        };

        let decision = self.ai_service.grid_decision(&self.bot.user_id, &system_prompt, &user_prompt).await;
        let action = self.handle_llm_result(&decision, &system_prompt, &user_prompt).await;

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
        let total_hold: f64 = self.levels.iter().map(|l| l.hold_quantity).sum();

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
            last_adjust_time: "N/A".to_string(),
            consecutive_losses: self.consecutive_losses,
            current_grid_config,
            position_base: total_hold,
            position_side: if total_hold > 0.0 { "long".to_string() } else if total_hold < 0.0 { "short".to_string() } else { "none".to_string() },
            entry_price: self.compute_weighted_avg_entry_price(),
            unrealized_pnl: self.compute_unrealized_pnl(),
            open_orders: self.format_open_orders(),
            funding_rate: snapshot.indicators.funding_rate,
            funding_next_time: "N/A".to_string(),
            event_flag: false,
            event_description: String::new(),
            trigger_reason: "scheduled_15m".to_string(),
            ind: snapshot.indicators,
        };

        let user_prompt = prompt::render_prompt(template, &ctx);
        let system_prompt = self.bot.system_prompt.as_deref().unwrap_or(LLM_RUNTIME_PROMPT).to_string();

        Some((system_prompt, user_prompt))
    }

/** 处理 LLM 调用结果，记录分析日志，返回最终 action

LLM 成功时记录结果并返回其 action，失败时回退到规则决策 */
    async fn handle_llm_result(
        &mut self,
        decision: &Option<GridDecision>,
        system_prompt: &str,
        user_prompt: &str,
    ) -> GridAction {
        match decision {
            Some(d) => {
                info!(bot_id = %self.bot.id, action = d.action.as_str(), reason = %d.reason, source = "llm", "LLM decision");

                let result = serde_json::json!({
                    "action": d.action.as_str(),
                    "reason": d.reason,
                    "upper_price": d.upper_price,
                    "lower_price": d.lower_price,
                });
                let _ = self.store.save_analysis_log(
                    self.bot.id, "periodic", system_prompt, user_prompt,
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
                    self.bot.id, "periodic", system_prompt, user_prompt,
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

/** 执行 AI 决策 */
    pub(crate) async fn execute_decision(&mut self, action: &GridAction, decision: Option<&GridDecision>) {
        match action {
            GridAction::PauseGrid => {
                self.pause_with_cancel("LLM decision").await;
            }
            GridAction::RunGrid => {
                if self.paused {
                    self.paused = false;
                    self.place_initial_orders().await;
                    info!(bot_id = %self.bot.id, "Grid resumed by decision");
                }
            }
            GridAction::ReducePosition => {
                let new_qty = self.bot.quantity_per_grid * 0.5;
                let _ = self.store.update_quantity_per_grid(self.bot.id, new_qty).await;
                self.bot.quantity_per_grid = new_qty;
                self.recalculate_levels();
                warn!(bot_id = %self.bot.id, new_qty, "Position reduced by decision");
            }
            GridAction::AdjustGrid { .. } => {
                if let Some(d) = decision {
                    self.adjust_grid(d.upper_price, d.lower_price, false).await;
                }
            }
            GridAction::Hold => {}
        }
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
        let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
            symbol: Some(self.bot.symbol.clone()),
        }).await;

        let mut updated = false;
        if let Some(upper) = new_upper {
            if upper > 0.0 && upper != self.bot.upper_price {
                self.bot.upper_price = upper;
                updated = true;
            }
        }
        if let Some(lower) = new_lower {
            if lower > 0.0 && lower != self.bot.lower_price {
                self.bot.lower_price = lower;
                updated = true;
            }
        }

        if !updated && !force_recalculate {
            debug!(bot_id = %self.bot.id, "adjust_grid: no parameter changes");
            return;
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

        self.levels = Self::calculate_levels(&self.bot);

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
                level.hold_quantity = old.hold_quantity;
                level.avg_buy_price = old.avg_buy_price;
                level.buy_filled = old.buy_filled;
                level.sell_filled = old.sell_filled;
                level.last_fill_price = old.last_fill_price;
            }
        }

        self.pending_orders.clear();
        info!(bot_id = %self.bot.id, grid_count = self.levels.len(), holdings_matched = holdings.len(), "Grid levels recalculated (preserved holdings by price)");
    }
}
