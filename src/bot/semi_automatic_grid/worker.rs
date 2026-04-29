use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sqlx::PgPool;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::engine::strategy::StrategyEngine;
use crate::bot::semi_automatic_grid::ai::{GridAiService, GridAction};
use crate::bot::semi_automatic_grid::types::{GridEvent, GridLevel, GridState};
use crate::models::GridBot;
use crate::engine::position::types::*;

/// 单个网格 bot 的执行 worker
pub struct GridWorker {
    bot: GridBot,
    db: PgPool,
    /// 策略引擎（用于获取实时价格）
    strategy_engine: Arc<StrategyEngine>,
    /// AI 决策服务（LLM 调用）
    ai_service: Arc<GridAiService>,
    /// Position Engine 命令通道
    pe_cmd_tx: mpsc::Sender<EngineCommand>,
    /// Position Engine 事件通道
    pe_event_rx: broadcast::Receiver<EngineEvent>,
    /// 网格事件广播
    event_tx: broadcast::Sender<GridEvent>,
    /// 网格层状态
    levels: Vec<GridLevel>,
    /// 当前价格
    current_price: f64,
    /// 统计
    total_pnl: f64,
    total_trades: i32,
    grid_filled_count: i32,
    /// 是否暂停（LLM 决策 pause_grid 或风控触发后设为 true）
    paused: bool,
    /// Fix #6: order_id -> (level_index, side) 的映射，替代价格比较
    order_level_map: HashMap<Uuid, (usize, String)>,
    /// Fix #5: 初始挂单范围（当前价格 ±N 层）
    initial_order_range: usize,
}

impl GridWorker {
    pub fn new(
        bot: GridBot,
        db: PgPool,
        strategy_engine: Arc<StrategyEngine>,
        ai_service: Arc<GridAiService>,
        pe_cmd_tx: mpsc::Sender<EngineCommand>,
        pe_event_rx: broadcast::Receiver<EngineEvent>,
        event_tx: broadcast::Sender<GridEvent>,
    ) -> Self {
        let levels = Self::calculate_levels(&bot);
        let current_price = 0.0;

        Self {
            bot,
            db,
            strategy_engine,
            ai_service,
            pe_cmd_tx,
            pe_event_rx,
            event_tx,
            levels,
            current_price,
            total_pnl: 0.0,
            total_trades: 0,
            grid_filled_count: 0,
            paused: false,
            order_level_map: HashMap::new(),
            initial_order_range: 3, // 初始在当前价格附近 ±3 层挂单
        }
    }

    /// 根据网格参数计算所有层级价格
    fn calculate_levels(bot: &GridBot) -> Vec<GridLevel> {
        if bot.grid_count <= 0 || bot.upper_price <= 0.0 || bot.lower_price <= 0.0 {
            return vec![];
        }
        let grid_spacing = (bot.upper_price - bot.lower_price) / bot.grid_count as f64;
        let profit_factor = 1.0 + bot.grid_profit_pct / 100.0;

        (0..bot.grid_count)
            .map(|i| {
                let buy_price = bot.lower_price + grid_spacing * (i as f64 + 0.5);
                let sell_price = buy_price * profit_factor;
                let quantity = if buy_price > 0.0 {
                    bot.quantity_per_grid / buy_price
                } else {
                    0.0
                };

                GridLevel {
                    level: i,
                    price: buy_price,
                    buy_price,
                    sell_price,
                    quantity,
                    buy_order_id: None,
                    sell_order_id: None,
                    buy_filled: false,
                    sell_filled: false,
                    hold_quantity: 0.0,
                }
            })
            .collect()
    }

    // ── Fix #1: 获取实时价格 ──

    /// 从交易所获取最新价格
    async fn fetch_current_price(&self) -> f64 {
        let exchange_key = format!("{}:Perpetual", self.bot.exchange);
        if let Some(exchange) = self.strategy_engine.get_exchange(&exchange_key) {
            match exchange.get_ticker(&self.bot.symbol).await {
                Ok(ticker) => {
                    if ticker.last > 0.0 {
                        return ticker.last;
                    }
                }
                Err(e) => {
                    debug!(
                        bot_id = %self.bot.id,
                        error = %e,
                        "Failed to fetch ticker"
                    );
                }
            }
        }
        // 回退：使用上次价格
        self.current_price
    }

    // ── 主运行循环 ──

    pub async fn run(&mut self, mut shutdown_rx: mpsc::Receiver<()>) {
        info!(
            bot_id = %self.bot.id,
            symbol = %self.bot.symbol,
            grid_count = self.bot.grid_count,
            "GridWorker starting"
        );

        if self.levels.is_empty() {
            error!(bot_id = %self.bot.id, "No grid levels calculated, check bot parameters");
            return;
        }

        // Fix #1: 启动时获取初始价格
        self.current_price = self.fetch_current_price().await;
        info!(
            bot_id = %self.bot.id,
            price = self.current_price,
            "Initial price fetched"
        );

        // 1. 加载历史成交
        self.load_existing_trades().await;

        // 2. Fix #5: 初始挂单（只在当前价格附近）
        self.place_initial_orders().await;

        let mut price_tick = tokio::time::interval(Duration::from_secs(5));
        // LLM 决策间隔
        let llm_interval_secs = if self.bot.dynamic_adjust {
            self.bot.adjust_interval_secs.max(60) as u64
        } else {
            u64::MAX
        };
        let mut llm_tick = tokio::time::interval(Duration::from_secs(llm_interval_secs));
        llm_tick.tick().await; // 跳过首次

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!(bot_id = %self.bot.id, "GridWorker shutting down");
                    break;
                }
                _ = price_tick.tick() => {
                    // Fix #1: 每次 tick 更新价格
                    self.current_price = self.fetch_current_price().await;
                    if !self.paused {
                        self.on_price_tick().await;
                    }
                }
                _ = llm_tick.tick() => {
                    if llm_interval_secs != u64::MAX {
                        self.on_llm_decision().await;
                    }
                }
                event = self.pe_event_rx.recv() => {
                    match event {
                        Ok(event) => self.on_pe_event(event).await,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(bot_id = %self.bot.id, lagged = n, "PE event lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!(bot_id = %self.bot.id, "PE event channel closed");
                            break;
                        }
                    }
                }
            }
        }

        self.save_stats().await;
    }

    // ── 历史成交 ──

    async fn load_existing_trades(&mut self) {
        let trades: Vec<crate::models::GridTrade> = sqlx::query_as(
            "SELECT * FROM qd_grid_trades WHERE bot_id = $1 AND status = 'filled' ORDER BY created_at",
        )
        .bind(self.bot.id)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        let trade_count = trades.len();
        for trade in trades {
            let level_idx = trade.grid_level as usize;
            if level_idx < self.levels.len() {
                if trade.side == "buy" {
                    self.levels[level_idx].buy_filled = true;
                    self.levels[level_idx].hold_quantity += trade.quantity;
                } else {
                    self.levels[level_idx].sell_filled = true;
                    self.levels[level_idx].hold_quantity -= trade.quantity;
                }
            }
            self.total_pnl += trade.pnl;
            self.total_trades += 1;
        }

        info!(
            bot_id = %self.bot.id,
            loaded_trades = trade_count,
            total_pnl = self.total_pnl,
            "Loaded existing grid trades"
        );
    }

    // ── Fix #5: 初始挂单只在当前价格附近 ──

    async fn place_initial_orders(&mut self) {
        if self.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "No current price, skipping initial orders");
            return;
        }

        // 找到当前价格所在的层级索引
        let current_level_idx = self.find_level_by_price(self.current_price);

        // 在当前价格下方 initial_order_range 层挂买单
        for i in 0..self.levels.len() {
            let level = &self.levels[i];
            // 只在当前价格以下且在范围内的空层挂买单
            if level.buy_price < self.current_price
                && !level.buy_filled
                && level.buy_order_id.is_none()
                && i.saturating_sub(current_level_idx) <= self.initial_order_range
            {
                self.place_buy_order(level).await;
            }
        }

        // 在持有仓位的层挂卖单
        for level in &self.levels {
            if level.hold_quantity > 0.0 && level.sell_order_id.is_none() {
                self.place_sell_order(level).await;
            }
        }

        info!(
            bot_id = %self.bot.id,
            current_level = current_level_idx,
            "Initial orders placed"
        );
    }

    /// 根据价格找到最近的网格层索引
    fn find_level_by_price(&self, price: f64) -> usize {
        let mut closest = 0;
        let mut min_diff = f64::MAX;
        for (i, level) in self.levels.iter().enumerate() {
            let diff = (price - level.buy_price).abs();
            if diff < min_diff {
                min_diff = diff;
                closest = i;
            }
        }
        closest
    }

    // ── 价格 tick ──

    async fn on_price_tick(&mut self) {
        if self.current_price <= 0.0 {
            return;
        }

        for level in &self.levels {
            // 价格低于买入价且未挂单 -> 挂买单
            if self.current_price < level.buy_price
                && !level.buy_filled
                && level.buy_order_id.is_none()
            {
                self.place_buy_order(level).await;
            }
            // 持有仓位且价格高于卖出价 -> 挂卖单
            if level.hold_quantity > 0.0
                && self.current_price >= level.sell_price
                && level.sell_order_id.is_none()
            {
                self.place_sell_order(level).await;
            }
        }

        self.broadcast_state();
    }

    // ── Fix #7: 风控集成 ──

    async fn on_pe_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::OrderPlaced { order } => {
                self.on_order_placed(&order).await;
            }
            EngineEvent::OrderFilled { order, trade } => {
                self.on_order_filled(&order, &trade).await;
            }
            EngineEvent::OrderCanceled { order } => {
                self.on_order_canceled(&order).await;
            }
            EngineEvent::OrderFailed { order_id, reason } => {
                warn!(
                    bot_id = %self.bot.id,
                    order_id = %order_id,
                    reason = %reason,
                    "Order failed"
                );
                self.clear_order_id(order_id);
            }
            EngineEvent::RiskAlert { level, message } => {
                warn!(
                    bot_id = %self.bot.id,
                    level = %level,
                    message = %message,
                    "Risk alert from PE"
                );
                // Fix #7: CloseAll 级别直接暂停网格
                if level == "CloseAll" {
                    if !self.paused {
                        self.paused = true;
                        let _ = self.pe_cmd_tx.send(EngineCommand::CancelAllOrders {
                            position_id: None,
                            symbol: Some(self.bot.symbol.clone()),
                        }).await;
                        warn!(
                            bot_id = %self.bot.id,
                            "Grid paused due to CloseAll risk alert"
                        );
                    }
                }
            }
            EngineEvent::LiquidationWarning { position_id, symbol, liquidation_price, current_price } => {
                warn!(
                    bot_id = %self.bot.id,
                    %position_id,
                    %symbol,
                    liquidation_price = liquidation_price,
                    current_price = current_price,
                    "Liquidation warning from PE"
                );
                // Fix #7: 强平预警也暂停网格
                if !self.paused {
                    self.paused = true;
                    let _ = self.pe_cmd_tx.send(EngineCommand::CancelAllOrders {
                        position_id: None,
                        symbol: Some(self.bot.symbol.clone()),
                    }).await;
                    warn!(
                        bot_id = %self.bot.id,
                        "Grid paused due to liquidation warning"
                    );
                }
            }
            _ => {
                debug!(
                    bot_id = %self.bot.id,
                    "Ignoring PE event: {:?}",
                    std::mem::discriminant(&event)
                );
            }
        }
    }

    // ── Fix #6: 用 HashMap 匹配 order_id ──

    async fn on_order_placed(&mut self, order: &Order) {
        // 通过 order_level_map 查找
        if let Some(&(level_idx, ref side)) = self.order_level_map.get(&order.id) {
            info!(
                bot_id = %self.bot.id,
                level = level_idx,
                side = %side,
                order_id = %order.id,
                "Grid order placed (via map)"
            );
            return;
        }

        // 回退：通过价格匹配（首次挂单时）
        if let Some(price) = order.fill_price.or(order.request_price) {
            let mut matched: Option<(usize, bool, String)> = None; // (idx, is_buy, side_str)

            for (idx, level) in self.levels.iter().enumerate() {
                let is_buy = order.side == Side::Buy
                    && (price - level.buy_price).abs() < level.buy_price * 0.001; // 0.1% 容差
                let is_sell = order.side == Side::Sell
                    && (price - level.sell_price).abs() < level.sell_price * 0.001;

                if is_buy || is_sell {
                    let side_str = if is_buy { "buy" } else { "sell" };
                    matched = Some((idx, is_buy, side_str.to_string()));
                    break;
                }
            }

            if let Some((idx, is_buy, side_str)) = matched {
                self.order_level_map.insert(order.id, (idx, side_str.clone()));

                if is_buy {
                    self.levels[idx].buy_order_id = Some(order.id);
                } else {
                    self.levels[idx].sell_order_id = Some(order.id);
                }

                info!(
                    bot_id = %self.bot.id,
                    level = self.levels[idx].level,
                    side = %side_str,
                    price = price,
                    order_id = %order.id,
                    "Grid order placed (via price)"
                );
            }
        }
    }

    async fn on_order_filled(&mut self, order: &Order, _trade: &Trade) {
        let side_str = match order.side {
            Side::Buy => "buy",
            Side::Sell => "sell",
        };

        // Fix #6: 优先通过 order_level_map 查找
        let matched_idx = if let Some(&(idx, ref side)) = self.order_level_map.get(&order.id) {
            if side == side_str {
                Some(idx)
            } else {
                None
            }
        } else {
            None
        };

        let idx = match matched_idx {
            Some(i) => i,
            None => {
                debug!(
                    bot_id = %self.bot.id,
                    order_id = %order.id,
                    "Order filled but no matching grid level"
                );
                return;
            }
        };

        let price = order.fill_price.unwrap_or(0.0);
        let level = &mut self.levels[idx];
        let is_buy_match = order.side == Side::Buy;
        let is_sell_match = !is_buy_match;
        let level_num = level.level;

        // 提取挂买单所需数据
        let rebuy_level = if is_sell_match {
            Some(GridLevel {
                level: level.level,
                price: level.price,
                buy_price: level.buy_price,
                sell_price: level.sell_price,
                quantity: level.quantity,
                buy_order_id: None,
                sell_order_id: None,
                buy_filled: false,
                sell_filled: false,
                hold_quantity: 0.0,
            })
        } else {
            None
        };

        if is_buy_match {
            level.buy_filled = true;
            level.buy_order_id = None;
            level.hold_quantity += order.filled;
        } else {
            level.sell_filled = true;
            level.sell_order_id = None;
            level.hold_quantity -= order.filled;
        }

        // 清除映射
        self.order_level_map.remove(&order.id);

        // 计算盈亏（卖出时）
        let pnl = if is_sell_match {
            let buy_cost = level.buy_price * order.filled;
            let sell_revenue = price * order.filled;
            sell_revenue - buy_cost
        } else {
            0.0
        };

        let hold = level.hold_quantity;

        self.total_pnl += pnl;
        self.total_trades += 1;
        self.grid_filled_count += 1;

        // 卖出成交后，重新在买入价挂买单
        if let Some(rebuy) = rebuy_level {
            self.place_buy_order(&rebuy).await;
        }

        // 记录到数据库
        self.record_trade(level_num, &side_str, price, order.filled, pnl).await;

        // 发送事件
        if is_sell_match {
            let _ = self.event_tx.send(GridEvent::GridTradeClosed {
                bot_id: self.bot.id,
                level: level_num,
                pnl,
            });
        }
        let _ = self.event_tx.send(GridEvent::GridFilled {
            bot_id: self.bot.id,
            level: level_num,
            side: side_str.to_string(),
            price,
            quantity: order.filled,
        });

        info!(
            bot_id = %self.bot.id,
            level = level_num,
            side = %side_str,
            price = price,
            quantity = order.filled,
            pnl = pnl,
            hold = hold,
            "Grid order filled"
        );
    }

    async fn on_order_canceled(&mut self, order: &Order) {
        self.clear_order_id(order.id);
        debug!(
            bot_id = %self.bot.id,
            order_id = %order.id,
            "Grid order canceled"
        );
    }

    fn clear_order_id(&mut self, order_id: Uuid) {
        // Fix #6: 同时清除映射和 level 中的引用
        self.order_level_map.remove(&order_id);
        for level in &mut self.levels {
            if level.buy_order_id == Some(order_id) {
                level.buy_order_id = None;
            }
            if level.sell_order_id == Some(order_id) {
                level.sell_order_id = None;
            }
        }
    }

    // ── 下单 ──

    async fn place_buy_order(&self, level: &GridLevel) {
        let cmd = EngineCommand::PlaceOrder {
            params: PlaceOrderParams {
                symbol: self.bot.symbol.clone(),
                side: Side::Buy,
                order_type: OrderType::Limit,
                amount: level.quantity,
                price: Some(level.buy_price),
                reduce_only: false,
                position_side: Some(PositionSide::Long),
            },
        };
        if let Err(e) = self.pe_cmd_tx.send(cmd).await {
            error!(
                bot_id = %self.bot.id,
                level = level.level,
                error = %e,
                "Failed to send buy order command"
            );
        }
    }

    async fn place_sell_order(&self, level: &GridLevel) {
        let cmd = EngineCommand::PlaceOrder {
            params: PlaceOrderParams {
                symbol: self.bot.symbol.clone(),
                side: Side::Sell,
                order_type: OrderType::Limit,
                amount: level.hold_quantity.min(level.quantity),
                price: Some(level.sell_price),
                reduce_only: true,
                position_side: Some(PositionSide::Long),
            },
        };
        if let Err(e) = self.pe_cmd_tx.send(cmd).await {
            error!(
                bot_id = %self.bot.id,
                level = level.level,
                error = %e,
                "Failed to send sell order command"
            );
        }
    }

    // ── 数据库 ──

    async fn record_trade(&self, level: i32, side: &str, price: f64, quantity: f64, pnl: f64) {
        let pnl_pct = if price > 0.0 {
            pnl / (price * quantity) * 100.0
        } else {
            0.0
        };

        let _ = sqlx::query(
            r#"INSERT INTO qd_grid_trades (bot_id, user_id, symbol, exchange, side, grid_level, price, quantity, pnl, pnl_pct, status)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'filled')"#,
        )
        .bind(self.bot.id)
        .bind(self.bot.user_id)
        .bind(&self.bot.symbol)
        .bind(&self.bot.exchange)
        .bind(side)
        .bind(level)
        .bind(price)
        .bind(quantity)
        .bind(pnl)
        .bind(pnl_pct)
        .execute(&self.db)
        .await;
    }

    async fn save_stats(&self) {
        let _ = sqlx::query(
            "UPDATE qd_grid_bots SET total_pnl = $2, total_trades = $3, grid_filled_count = $4, updated_at = NOW() WHERE id = $1",
        )
        .bind(self.bot.id)
        .bind(self.total_pnl)
        .bind(self.total_trades)
        .bind(self.grid_filled_count)
        .execute(&self.db)
        .await;
    }

    // ── 状态广播 ──

    fn broadcast_state(&self) {
        let state = GridState {
            bot_id: self.bot.id,
            symbol: self.bot.symbol.clone(),
            exchange: self.bot.exchange.clone(),
            levels: self.levels.clone(),
            current_price: self.current_price,
            total_pnl: self.total_pnl,
            total_trades: self.total_trades,
            grid_filled_count: self.grid_filled_count,
            last_tick_at: Utc::now(),
        };
        let _ = self.event_tx.send(GridEvent::StatusUpdate {
            bot_id: self.bot.id,
            state,
        });
    }

    // ── LLM 决策 ──

    async fn on_llm_decision(&mut self) {
        info!(bot_id = %self.bot.id, "LLM decision tick");

        let filled_count = self.levels.iter().filter(|l| l.buy_filled).count();
        let total_hold: f64 = self.levels.iter().map(|l| l.hold_quantity).sum();
        let user_prompt = format!(
            r#"## 当前状态
- 交易对: {} ({})
- 市场状态: {}
- 当前价格: {:.2}
- 网格区间: {:.2} - {:.2}（{} 层）
- 已填充: {}/{} 层
- 持仓数量: {:.6}
- 累计盈亏: {:.2} USDT
- 累计交易: {} 笔
- 暂停状态: {}

## 可选操作
- run_grid: 继续运行（如果已暂停则恢复）
- pause_grid: 暂停（撤销所有挂单）
- adjust_grid: 调整网格参数（需返回新的 upper_price 和 lower_price）
- reduce_position: 减半仓位（quantity_per_grid 减半）
- hold: 保持当前状态

请返回 JSON: {{ "action": "run_grid|pause_grid|adjust_grid|reduce_position|hold", "reason": "简短理由", "upper_price": null, "lower_price": null }}"#,
            self.bot.symbol,
            self.bot.exchange,
            self.bot.market_regime.as_deref().unwrap_or("unknown"),
            self.current_price,
            self.bot.lower_price,
            self.bot.upper_price,
            self.bot.grid_count,
            filled_count,
            self.bot.grid_count,
            total_hold,
            self.total_pnl,
            self.total_trades,
            self.paused,
        );

        let system_prompt = self.bot.system_prompt.as_deref()
            .unwrap_or(LLM_RUNTIME_PROMPT);

        // 尝试 LLM 决策，失败时回退到规则
        let decision = self.ai_service
            .grid_decision(&self.bot.user_id, system_prompt, &user_prompt)
            .await;

        let action = match decision {
            Some(ref d) => {
                info!(
                    bot_id = %self.bot.id,
                    action = d.action.as_str(),
                    reason = %d.reason,
                    source = "llm",
                    "LLM decision result"
                );
                d.action.clone()
            }
            None => {
                let rule_action = self.simple_rule_decision();
                info!(
                    bot_id = %self.bot.id,
                    action = rule_action.as_str(),
                    source = "rule_fallback",
                    "Rule-based decision result"
                );
                rule_action
            }
        };

        self.execute_decision(&action, decision.as_ref()).await;

        let _ = sqlx::query(
            "UPDATE qd_grid_bots SET last_adjusted_at = NOW() WHERE id = $1"
        )
        .bind(self.bot.id)
        .execute(&self.db)
        .await;
    }

    /// 执行 LLM/规则决策
    async fn execute_decision(&mut self, action: &GridAction, decision: Option<&crate::bot::semi_automatic_grid::ai::GridDecision>) {
        match action {
            GridAction::PauseGrid => {
                if !self.paused {
                    self.paused = true;
                    let _ = self.pe_cmd_tx.send(EngineCommand::CancelAllOrders {
                        position_id: None,
                        symbol: Some(self.bot.symbol.clone()),
                    }).await;
                    warn!(bot_id = %self.bot.id, "Grid paused by decision");
                }
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
                let _ = sqlx::query(
                    "UPDATE qd_grid_bots SET quantity_per_grid = $2, updated_at = NOW() WHERE id = $1"
                )
                .bind(self.bot.id)
                .bind(new_qty)
                .execute(&self.db)
                .await;
                self.bot.quantity_per_grid = new_qty;
                self.levels = Self::calculate_levels(&self.bot);
                warn!(bot_id = %self.bot.id, new_qty = new_qty, "Position reduced by decision");
            }
            GridAction::AdjustGrid { .. } => {
                // Fix #3: 热调整网格参数
                if let Some(d) = decision {
                    self.adjust_grid(d.upper_price, d.lower_price).await;
                }
            }
            GridAction::Hold => {} // 无操作
        }
    }

    /// Fix #3: 热调整网格 — 撤销所有挂单，更新参数，重新计算层级，重新挂单
    async fn adjust_grid(&mut self, new_upper: Option<f64>, new_lower: Option<f64>) {
        // 1. 撤销所有挂单
        let _ = self.pe_cmd_tx.send(EngineCommand::CancelAllOrders {
            position_id: None,
            symbol: Some(self.bot.symbol.clone()),
        }).await;

        // 2. 更新参数（仅更新 LLM 返回的非零值）
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

        if !updated {
            debug!(bot_id = %self.bot.id, "adjust_grid: no parameter changes");
            return;
        }

        // 3. 持久化到数据库
        let _ = sqlx::query(
            "UPDATE qd_grid_bots SET upper_price = $2, lower_price = $3, updated_at = NOW() WHERE id = $1"
        )
        .bind(self.bot.id)
        .bind(self.bot.upper_price)
        .bind(self.bot.lower_price)
        .execute(&self.db)
        .await;

        // 4. 重新计算网格层
        self.recalculate_levels();

        // 5. 重新挂单
        if !self.paused {
            self.place_initial_orders().await;
        }

        info!(
            bot_id = %self.bot.id,
            new_upper = self.bot.upper_price,
            new_lower = self.bot.lower_price,
            grid_count = self.levels.len(),
            "Grid adjusted successfully"
        );

        let _ = self.event_tx.send(GridEvent::BotError {
            bot_id: self.bot.id,
            error: format!(
                "Grid adjusted: upper={:.2}, lower={:.2}, levels={}",
                self.bot.upper_price, self.bot.lower_price, self.levels.len()
            ),
        });
    }

    /// 简单规则决策（LLM 不可用时的回退逻辑）
    fn simple_rule_decision(&self) -> GridAction {
        if self.current_price > self.bot.upper_price * 1.02 {
            return GridAction::PauseGrid;
        }
        if self.current_price < self.bot.lower_price * 0.98 {
            return GridAction::PauseGrid;
        }
        if self.paused
            && self.current_price >= self.bot.lower_price
            && self.current_price <= self.bot.upper_price
        {
            return GridAction::RunGrid;
        }
        GridAction::Hold
    }

    /// 重新计算网格层（热调整时调用）
    pub fn recalculate_levels(&mut self) {
        self.levels = Self::calculate_levels(&self.bot);
        self.order_level_map.clear();
        info!(
            bot_id = %self.bot.id,
            grid_count = self.levels.len(),
            "Grid levels recalculated"
        );
    }
}

const LLM_RUNTIME_PROMPT: &str = r#"你是一位正在管理加密货币网格交易机器人的 AI 助手。你的职责是根据当前市场状态和机器人运行数据，做出最优决策。

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
