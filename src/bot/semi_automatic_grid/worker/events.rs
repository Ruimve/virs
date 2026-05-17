use chrono::Utc;
use tracing::{info, warn};

use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::{GridEvent, GridState};
use crate::bot::semi_automatic_grid::utils::holdings::{apply_fill_to_level, calculate_fill_pnl, is_close_trade};
use crate::bot::semi_automatic_grid::worker::GridWorker;

impl GridWorker {
/** 加载历史成交记录，恢复网格层持仓状态

从数据库加载该 bot 的所有历史成交，按价格匹配到网格层，
重建每层的持仓量、均价、已填充标志等状态 */
    pub(crate) async fn load_existing_trades(&mut self) {
        let trades = self.store.load_trades(self.bot.id).await.unwrap_or_default();
        let max_dist = self.grid_spacing();

        let trade_count = trades.len();
        for trade in trades {
            if let Some(level_idx) = self.find_level_by_price_within(trade.price, max_dist) {
                apply_fill_to_level(&mut self.levels[level_idx], &trade.side, trade.price, trade.quantity);
            } else {
                warn!(
                    bot_id = %self.bot.id, trade_price = trade.price, grid_level = trade.grid_level,
                    "Trade could not be matched to any grid level by price, skipping"
                );
            }
            self.total_pnl += trade.pnl;
            self.total_trades += 1;
            self.update_consecutive_losses(trade.pnl);
        }

        info!(
            bot_id = %self.bot.id,
            loaded_trades = trade_count,
            total_pnl = self.total_pnl,
            "Loaded existing grid trades"
        );

        self.reset_completed_cycles();
    }

/** 重置已完成买卖周期的层级状态，使其可以重新开仓 */
    fn reset_completed_cycles(&mut self) {
        for level in &mut self.levels {
            let cycle_complete = if level.side == "buy" {
                level.buy_filled && level.sell_filled && level.hold_quantity <= 0.0
            } else {
                level.sell_filled && level.buy_filled && level.hold_quantity >= 0.0
            };
            if cycle_complete {
                level.buy_filled = false;
                level.sell_filled = false;
                level.buy_order_id = None;
                level.sell_order_id = None;
                level.hold_quantity = 0.0;
                level.avg_buy_price = 0.0;
            }
        }
    }

/** 计算网格间距（用于价格匹配的最大距离） */
    pub(crate) fn grid_spacing(&self) -> f64 {
        if self.levels.len() > 1 {
            (self.bot.upper_price - self.bot.lower_price) / self.levels.len() as f64
        } else {
            0.0
        }
    }

/** 更新连续亏损计数 */
    pub(crate) fn update_consecutive_losses(&mut self, pnl: f64) {
        if pnl < 0.0 {
            self.consecutive_losses += 1;
        } else if pnl > 0.0 {
            self.consecutive_losses = 0;
        }
    }

/** 根据价格找到最近的网格层索引 */
    pub(crate) fn find_level_by_price(&self, price: f64) -> usize {
        let mut closest = 0;
        let mut min_diff = f64::MAX;
        for (i, level) in self.levels.iter().enumerate() {
            let diff = (price - level.price).abs();
            if diff < min_diff {
                min_diff = diff;
                closest = i;
            }
        }
        closest
    }

/** 根据价格找到最近的网格层索引，要求距离不超过 max_dist */
    pub(crate) fn find_level_by_price_within(&self, price: f64, max_dist: f64) -> Option<usize> {
        let (idx, dist) = self.levels.iter().enumerate()
            .map(|(i, l)| (i, (l.price - price).abs()))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;
        if max_dist <= 0.0 || dist <= max_dist {
            Some(idx)
        } else {
            None
        }
    }

/** 计算未实现盈亏

基于各层持仓量和均价，与当前价格比较计算浮动盈亏 */
    pub(crate) fn compute_unrealized_pnl(&self) -> f64 {
        if self.current_price <= 0.0 {
            return 0.0;
        }
        self.levels.iter()
            .filter(|l| l.hold_quantity.abs() > 0.0 && l.avg_buy_price > 0.0)
            .map(|l| {
                if l.hold_quantity > 0.0 {
                    (self.current_price - l.avg_buy_price) * l.hold_quantity
                } else {
                    (l.avg_buy_price - self.current_price) * l.hold_quantity.abs()
                }
            })
            .sum()
    }

/** 计算所有持仓的加权平均入场价格 */
    pub(crate) fn compute_weighted_avg_entry_price(&self) -> f64 {
        let mut total_cost = 0.0;
        let mut total_qty = 0.0;
        for level in &self.levels {
            if level.hold_quantity.abs() > 0.0 && level.avg_buy_price > 0.0 {
                total_cost += level.avg_buy_price * level.hold_quantity.abs();
                total_qty += level.hold_quantity.abs();
            }
        }
        if total_qty > 0.0 { total_cost / total_qty } else { 0.0 }
    }

/** 格式化当前挂单信息，用于 AI prompt 中 {open_orders} 占位符 */
    pub(crate) fn format_open_orders(&self) -> String {
        let orders: Vec<String> = self.levels.iter()
            .filter(|l| l.buy_order_id.is_some() || l.sell_order_id.is_some())
            .map(|l| {
                let mut parts = Vec::new();
                if let Some(id) = l.buy_order_id {
                    parts.push(format!("{{level:{}, side:buy, price:{:.2}, qty:{:.6}, id:{}}}", l.level, l.buy_price, l.quantity, id));
                }
                if let Some(id) = l.sell_order_id {
                    parts.push(format!("{{level:{}, side:sell, price:{:.2}, qty:{:.6}, id:{}}}", l.level, l.sell_price, l.quantity, id));
                }
                parts.join(", ")
            })
            .collect();
        if orders.is_empty() { "[]".to_string() } else { format!("[{}]", orders.join(", ")) }
    }

/** 持久化统计数据到数据库 */
    pub(crate) async fn save_stats(&self) {
        let levels_json = serde_json::to_value(&self.levels).ok();
        let _ = self.store.save_stats(self.bot.id, self.total_pnl, self.compute_unrealized_pnl(), self.total_trades, self.grid_filled_count, levels_json.as_ref()).await;
    }

/** 广播当前网格状态 */
    pub(crate) fn broadcast_state(&self) {
        let state = GridState {
            bot_id: self.bot.id,
            symbol: self.bot.symbol.clone(),
            exchange: self.bot.exchange.clone(),
            levels: self.levels.clone(),
            current_price: self.current_price,
            total_pnl: self.total_pnl,
            unrealized_pnl: self.compute_unrealized_pnl(),
            total_trades: self.total_trades,
            grid_filled_count: self.grid_filled_count,
            last_tick_at: Utc::now(),
        };
        let _ = self.grid_event_tx.send(GridEvent::StatusUpdate { bot_id: self.bot.id, state });
    }

/** 记录单笔交易到数据库 */
    async fn record_trade(&self, level: i32, side: &str, price: f64, quantity: f64, pnl: f64) {
        let pnl_pct = if price > 0.0 { pnl / (price * quantity) * 100.0 } else { 0.0 };
        let _ = self.store.record_trade(
            self.bot.id, self.bot.user_id, &self.bot.symbol, &self.bot.exchange,
            side, level, price, quantity, pnl, pnl_pct,
        ).await;
    }

/** 处理外部订单事件 */
    pub(crate) async fn on_order_event(&mut self, event: OrderEvent) {
        match event {
            OrderEvent::OrderPlaced { order } => {
                if order.symbol != self.bot.symbol {
                    return;
                }
                self.on_order_placed(&order).await;
            }
            OrderEvent::OrderFilled { order } => {
                if order.symbol != self.bot.symbol {
                    return;
                }
                self.on_order_filled(&order).await;
            }
            OrderEvent::OrderCanceled { order_id, symbol } => {
                if let Some(ref sym) = symbol {
                    if sym != &self.bot.symbol {
                        return;
                    }
                }
                self.on_order_canceled(order_id).await;
            }
            OrderEvent::OrderFailed { order_id, reason } => {
                warn!(bot_id = %self.bot.id, order_id = %order_id, reason = %reason, "Order failed");
                self.clear_order_id(order_id);
            }
            OrderEvent::RiskAlert { level, message } => {
                warn!(bot_id = %self.bot.id, level = %level, message = %message, "Risk alert");
                if level == "CloseAll" {
                    self.pause_with_cancel("CloseAll risk alert").await;
                }
            }
            OrderEvent::LiquidationWarning { symbol, liquidation_price, current_price } => {
                warn!(
                    bot_id = %self.bot.id, %symbol,
                    liquidation_price, current_price,
                    "Liquidation warning"
                );
                self.pause_with_cancel("liquidation warning").await;
            }
        }
    }

/** 暂停网格并取消所有挂单 */
    pub(crate) async fn pause_with_cancel(&mut self, reason: &str) {
        if !self.paused {
            self.paused = true;
            let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
                symbol: Some(self.bot.symbol.clone()),
            }).await;
            warn!(bot_id = %self.bot.id, "Grid paused due to {}", reason);
        }
    }

/** 处理订单已挂出事件

优先通过 client_order_id 解析层级映射，回退到 order_id 反查 */
    pub(crate) async fn on_order_placed(&mut self, order: &OrderInfo) {
        if let Some(ref coi) = order.client_order_id {
            if let Some((level_idx, side)) = Self::parse_client_order_id(coi) {
                if level_idx < self.levels.len() {
                    self.pending_orders.remove(&(level_idx, side.clone()));

                    if side == "buy" {
                        self.levels[level_idx].buy_order_id = Some(order.id);
                    } else {
                        self.levels[level_idx].sell_order_id = Some(order.id);
                    }

                    info!(
                        bot_id = %self.bot.id, level = self.levels[level_idx].level,
                        side = %side, order_id = %order.id,
                        "Grid order placed"
                    );
                    return;
                }
            }
        }

        if let Some((level_idx, side)) = self.find_level_by_order_id(order.id) {
            info!(bot_id = %self.bot.id, level = level_idx, side = %side, order_id = %order.id, "Grid order placed (via order_id lookup)");
        }
    }

/** 解析 client_order_id 格式 "grid:{bot_id}:{level_idx}:{side}"

不再比较 bot_id（同一 worker 只会收到自己的事件），避免 Uuid→String 转换开销 */
    fn parse_client_order_id(coi: &str) -> Option<(usize, String)> {
        let parts: Vec<&str> = coi.splitn(4, ':').collect();
        if parts.len() == 4 && parts[0] == "grid" {
            if let Ok(level_idx) = parts[2].parse::<usize>() {
                let side = parts[3].to_string();
                return Some((level_idx, side));
            }
        }
        None
    }

/** 处理订单成交事件

更新网格层持仓状态、计算盈亏、触发反向挂单、记录交易 */
    pub(crate) async fn on_order_filled(&mut self, order: &OrderInfo) {
        let side_str = order.side.as_str();

        let idx = match self.find_level_by_order_id(order.id) {
            Some((i, ref side)) if side == side_str => i,
            _ => {
                warn!(bot_id = %self.bot.id, order_id = %order.id, side = %side_str, "Order filled but not matched to any grid level");
                return;
            }
        };

        let price = order.fill_price.unwrap_or(0.0);
        let level_side = self.levels[idx].side.clone();
        let level_num = self.levels[idx].level;
        let entry_price = self.levels[idx].avg_buy_price;

        apply_fill_to_level(&mut self.levels[idx], side_str, price, order.filled);

        self.pending_orders.remove(&(idx, side_str.to_string()));

        let pnl = calculate_fill_pnl(&level_side, side_str, entry_price, price, order.filled);
        let hold = self.levels[idx].hold_quantity;
        self.total_pnl += pnl;
        self.total_trades += 1;
        self.grid_filled_count += 1;
        self.update_consecutive_losses(pnl);

        self.place_reverse_order_if_cycle_complete(idx, &level_side, side_str, hold).await;

        self.record_trade(level_num, side_str, price, order.filled, pnl).await;

        if is_close_trade(&level_side, side_str) {
            let _ = self.grid_event_tx.send(GridEvent::GridTradeClosed { bot_id: self.bot.id, level: level_num, pnl });
        }
        let _ = self.grid_event_tx.send(GridEvent::GridFilled {
            bot_id: self.bot.id, level: level_num, side: side_str.to_string(), price, quantity: order.filled,
        });

        self.save_stats().await;

        info!(
            bot_id = %self.bot.id, level = level_num, side = %side_str,
            price, quantity = order.filled, pnl, hold,
            "Grid order filled"
        );
    }

/** 买卖周期完成后，自动挂反向单重新开仓 */
    async fn place_reverse_order_if_cycle_complete(
        &mut self,
        idx: usize,
        level_side: &str,
        trade_side: &str,
        hold: f64,
    ) {
        let cycle_complete = if level_side == "buy" {
            is_close_trade(level_side, trade_side) && hold <= 0.0
        } else {
            is_close_trade(level_side, trade_side) && hold >= 0.0
        };

        if !cycle_complete {
            return;
        }

        let reset_level = self.levels[idx].reset_for_relist();
        self.levels[idx] = reset_level.clone();
        if level_side == "buy" {
            self.place_buy_order(&reset_level).await;
        } else {
            self.place_sell_order(&reset_level).await;
        }
    }

/** 处理订单取消事件 */
    pub(crate) async fn on_order_canceled(&mut self, order_id: uuid::Uuid) {
        self.clear_order_id(order_id);
    }

/** 清除指定订单在网格层中的记录

通过 find_level_by_order_id 反查层级，清除 order_id 和 pending 标记 */
    pub(crate) fn clear_order_id(&mut self, order_id: uuid::Uuid) {
        if let Some((idx, side)) = self.find_level_by_order_id(order_id) {
            if side == "buy" {
                self.levels[idx].buy_order_id = None;
            } else {
                self.levels[idx].sell_order_id = None;
            }
            self.pending_orders.remove(&(idx, side));
        }
    }
}
