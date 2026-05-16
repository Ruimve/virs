use chrono::Utc;
use tracing::{info, warn};

use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::{GridEvent, GridLevel, GridState};
use crate::bot::semi_automatic_grid::worker::GridWorker;

impl GridWorker {
    /// 加载历史成交记录，恢复网格层持仓状态
    ///
    /// 从数据库加载该 bot 的所有历史成交，按价格匹配到网格层，
    /// 重建每层的持仓量、均价、已填充标志等状态
    pub(crate) async fn load_existing_trades(&mut self) {
        let trades = self.store.load_trades(self.bot.id).await.unwrap_or_default();

        /* 计算最大匹配距离（半格间距），用于将成交匹配到网格层 */
        let max_dist = if self.levels.len() > 1 {
            (self.bot.upper_price - self.bot.lower_price) / self.levels.len() as f64
        } else {
            0.0
        };

        let trade_count = trades.len();
        for trade in trades {
            let level_idx = self.find_level_by_price_within(trade.price, max_dist);
            if let Some(level_idx) = level_idx {
                let level_side = self.levels[level_idx].side.clone();
                if level_side == "buy" {
                    if trade.side == "buy" {
                        /* buy 层买入：加权平均计算新均价 */
                        let level = &mut self.levels[level_idx];
                        let old_total = level.avg_buy_price * level.hold_quantity;
                        let new_total = old_total + trade.price * trade.quantity;
                        level.hold_quantity += trade.quantity;
                        level.avg_buy_price = if level.hold_quantity > 0.0 {
                            new_total / level.hold_quantity
                        } else {
                            0.0
                        };
                        level.buy_filled = true;
                        level.last_fill_price = Some(trade.price);
                    } else {
                        /* buy 层卖出：减少持仓量 */
                        let level = &mut self.levels[level_idx];
                        level.hold_quantity = (level.hold_quantity - trade.quantity).max(0.0);
                        level.sell_filled = true;
                        level.last_fill_price = Some(trade.price);
                    }
                } else {
                    if trade.side == "sell" {
                        /* sell 层卖出开空：加权平均计算新均价 */
                        let level = &mut self.levels[level_idx];
                        let old_total = level.avg_buy_price * level.hold_quantity.abs();
                        let new_total = old_total + trade.price * trade.quantity;
                        level.hold_quantity -= trade.quantity;
                        level.avg_buy_price = if level.hold_quantity.abs() > 0.0 {
                            new_total / level.hold_quantity.abs()
                        } else {
                            0.0
                        };
                        level.sell_filled = true;
                        level.last_fill_price = Some(trade.price);
                    } else {
                        /* sell 层买入平空：减少空头持仓 */
                        let level = &mut self.levels[level_idx];
                        level.hold_quantity = (level.hold_quantity + trade.quantity).min(0.0);
                        level.buy_filled = true;
                        level.last_fill_price = Some(trade.price);
                    }
                }
            } else {
                warn!(
                    bot_id = %self.bot.id, trade_price = trade.price, grid_level = trade.grid_level,
                    "Trade could not be matched to any grid level by price, skipping"
                );
            }
            self.total_pnl += trade.pnl;
            self.total_trades += 1;
            /* 更新连续亏损计数 */
            if trade.pnl < 0.0 {
                self.consecutive_losses += 1;
            } else if trade.pnl > 0.0 {
                self.consecutive_losses = 0;
            }
        }

        info!(
            bot_id = %self.bot.id,
            loaded_trades = trade_count,
            total_pnl = self.total_pnl,
            "Loaded existing grid trades"
        );

        /* 重置已完成周期的层级状态，使其可以重新开仓 */
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

    /// 根据价格找到最近的网格层索引
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

    /// 根据价格找到最近的网格层索引，要求距离不超过 max_dist
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

    /// 计算未实现盈亏
    ///
    /// 基于各层持仓量和均价，与当前价格比较计算浮动盈亏
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

    /// 计算所有持仓的加权平均入场价格
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

    /// 格式化当前挂单信息，用于 AI prompt 中 {open_orders} 占位符
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

    /// 持久化统计数据到数据库
    pub(crate) async fn save_stats(&self) {
        let _ = self.store.save_stats(self.bot.id, self.total_pnl, self.compute_unrealized_pnl(), self.total_trades, self.grid_filled_count).await;
    }

    /// 广播当前网格状态
    ///
    /// 将所有层级、价格、盈亏等信息打包为 GridState 并通过事件通道广播
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

    /// 记录单笔交易到数据库
    async fn record_trade(&self, level: i32, side: &str, price: f64, quantity: f64, pnl: f64) {
        let pnl_pct = if price > 0.0 { pnl / (price * quantity) * 100.0 } else { 0.0 };
        let _ = self.store.record_trade(
            self.bot.id, self.bot.user_id, &self.bot.symbol, &self.bot.exchange,
            side, level, price, quantity, pnl, pnl_pct,
        ).await;
    }

    /// 处理外部订单事件
    ///
    /// 根据事件类型分发到对应的处理函数
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

    /// 暂停网格并取消所有挂单
    pub(crate) async fn pause_with_cancel(&mut self, reason: &str) {
        if !self.paused {
            self.paused = true;
            let _ = self.order_executor.send_command(OrderCommand::CancelAllOrders {
                symbol: Some(self.bot.symbol.clone()),
            }).await;
            warn!(bot_id = %self.bot.id, "Grid paused due to {}", reason);
        }
    }

    /// 处理订单已挂出事件
    ///
    /// 通过 order_level_map 或 client_order_id 将订单与网格层关联
    pub(crate) async fn on_order_placed(&mut self, order: &OrderInfo) {
        if let Some(&(level_idx, ref side)) = self.order_level_map.get(&order.id) {
            info!(bot_id = %self.bot.id, level = level_idx, side = %side, order_id = %order.id, "Grid order placed (via map)");
            return;
        }

        if let Some(ref coi) = order.client_order_id {
            if let Some((level_idx, side)) = Self::parse_client_order_id(coi, &self.bot.id) {
                if level_idx < self.levels.len() {
                    self.pending_orders.remove(&(level_idx, side.clone()));
                    self.order_level_map.insert(order.id, (level_idx, side.clone()));

                    if side == "buy" {
                        self.levels[level_idx].buy_order_id = Some(order.id);
                    } else {
                        self.levels[level_idx].sell_order_id = Some(order.id);
                    }

                    info!(
                        bot_id = %self.bot.id, level = self.levels[level_idx].level,
                        side = %side, order_id = %order.id,
                        "Grid order placed (via client_order_id)"
                    );
                    return;
                }
            }
        }
    }

    /// 解析 client_order_id 格式 "grid:{bot_id}:{level_idx}:{side}"
    fn parse_client_order_id(coi: &str, bot_id: &uuid::Uuid) -> Option<(usize, String)> {
        let parts: Vec<&str> = coi.splitn(4, ':').collect();
        if parts.len() == 4 && parts[0] == "grid" {
            if parts[1] != bot_id.to_string() {
                return None;
            }
            if let Ok(level_idx) = parts[2].parse::<usize>() {
                let side = parts[3].to_string();
                return Some((level_idx, side));
            }
        }
        None
    }

    /// 处理订单成交事件
    ///
    /// 更新网格层持仓状态、计算盈亏、触发反向挂单、记录交易
    pub(crate) async fn on_order_filled(&mut self, order: &OrderInfo) {
        let side_str = order.side.as_str();

        /* 通过 order_level_map 查找匹配的网格层 */
        let matched_idx = if let Some(&(idx, ref side)) = self.order_level_map.get(&order.id) {
            if side == side_str { Some(idx) } else { None }
        } else {
            None
        };

        let idx = match matched_idx {
            Some(i) => i,
            None => {
                return;
            }
        };

        let price = order.fill_price.unwrap_or(0.0);
        let level = &mut self.levels[idx];
        let is_buy_match = order.side == OrderSide::Buy;
        let is_sell_match = !is_buy_match;
        let level_num = level.level;
        let level_side = level.side.clone();
        let entry_price = level.avg_buy_price;

        /* 根据层级方向和成交方向更新持仓 */
        if level_side == "buy" {
            if is_buy_match {
                let old_total = level.avg_buy_price * level.hold_quantity;
                let new_total = old_total + price * order.filled;
                level.hold_quantity += order.filled;
                level.avg_buy_price = if level.hold_quantity > 0.0 {
                    new_total / level.hold_quantity
                } else {
                    0.0
                };
                level.buy_filled = true;
                level.buy_order_id = None;
                level.last_fill_price = Some(price);
            } else {
                level.sell_filled = true;
                level.sell_order_id = None;
                level.hold_quantity = (level.hold_quantity - order.filled).max(0.0);
                level.last_fill_price = Some(price);
            }
        } else {
            if is_sell_match {
                let old_total = level.avg_buy_price * level.hold_quantity.abs();
                let new_total = old_total + price * order.filled;
                level.hold_quantity -= order.filled;
                level.avg_buy_price = if level.hold_quantity.abs() > 0.0 {
                    new_total / level.hold_quantity.abs()
                } else {
                    0.0
                };
                level.sell_filled = true;
                level.sell_order_id = None;
                level.last_fill_price = Some(price);
            } else {
                level.buy_filled = true;
                level.buy_order_id = None;
                level.hold_quantity = (level.hold_quantity + order.filled).min(0.0);
                level.last_fill_price = Some(price);
            }
        }

        self.order_level_map.remove(&order.id);
        self.pending_orders.remove(&(idx, side_str.to_string()));

        /* 计算本次成交的已实现盈亏 */
        let pnl = if level_side == "buy" {
            if is_sell_match && entry_price > 0.0 {
                let buy_cost = entry_price * order.filled;
                let sell_revenue = price * order.filled;
                sell_revenue - buy_cost
            } else {
                0.0
            }
        } else {
            if is_buy_match && entry_price > 0.0 {
                let sell_revenue = entry_price * order.filled;
                let buy_cost = price * order.filled;
                sell_revenue - buy_cost
            } else {
                0.0
            }
        };

        let hold = self.levels[idx].hold_quantity;
        self.total_pnl += pnl;
        self.total_trades += 1;
        self.grid_filled_count += 1;

        /* 更新连续亏损计数 */
        if pnl < 0.0 {
            self.consecutive_losses += 1;
        } else if pnl > 0.0 {
            self.consecutive_losses = 0;
        }

        /* 完成一个买卖周期后，自动挂反向单 */
        if level_side == "buy" && is_sell_match && hold <= 0.0 {
            let rebuy = GridLevel {
                level: level_num, price: self.levels[idx].price,
                side: self.levels[idx].side.clone(),
                buy_price: self.levels[idx].buy_price, sell_price: self.levels[idx].sell_price,
                quantity: self.levels[idx].quantity, buy_order_id: None, sell_order_id: None,
                buy_filled: false, sell_filled: false, hold_quantity: 0.0,
                avg_buy_price: 0.0, last_fill_price: None,
            };
            self.place_buy_order(&rebuy).await;
        } else if level_side == "sell" && is_buy_match && hold >= 0.0 {
            let reoffer = GridLevel {
                level: level_num, price: self.levels[idx].price,
                side: self.levels[idx].side.clone(),
                buy_price: self.levels[idx].buy_price, sell_price: self.levels[idx].sell_price,
                quantity: self.levels[idx].quantity, buy_order_id: None, sell_order_id: None,
                buy_filled: false, sell_filled: false, hold_quantity: 0.0,
                avg_buy_price: 0.0, last_fill_price: None,
            };
            self.place_sell_order(&reoffer).await;
        }

        self.record_trade(level_num, side_str, price, order.filled, pnl).await;

        /* 广播交易事件 */
        if is_sell_match {
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

    /// 处理订单取消事件
    pub(crate) async fn on_order_canceled(&mut self, order_id: uuid::Uuid) {
        self.clear_order_id(order_id);
    }

    /// 清除指定订单在网格层和映射表中的记录
    pub(crate) fn clear_order_id(&mut self, order_id: uuid::Uuid) {
        if let Some((idx, side)) = self.order_level_map.remove(&order_id) {
            self.pending_orders.remove(&(idx, side));
        }
        for level in &mut self.levels {
            if level.buy_order_id == Some(order_id) {
                level.buy_order_id = None;
                self.pending_orders.remove(&(level.level as usize, "buy".to_string()));
            }
            if level.sell_order_id == Some(order_id) {
                level.sell_order_id = None;
                self.pending_orders.remove(&(level.level as usize, "sell".to_string()));
            }
        }
    }
}
