use tracing::{info, warn};

use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::GridLevel;
use crate::bot::semi_automatic_grid::worker::GridWorker;

impl GridWorker {
    /// 挂初始订单
    ///
    /// 在当前价格附近 ±initial_order_range 层范围内挂买卖单，
    /// 同时为已有持仓的层级挂平仓单
    pub(crate) async fn place_initial_orders(&mut self) {
        if self.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "No current price, skipping initial orders");
            return;
        }

        let current_level_idx = self.find_level_by_price(self.current_price);

        /* 挂 buy 层初始买单 */
        let buy_init_levels: Vec<GridLevel> = self.levels.iter().enumerate()
            .filter(|(i, level)| {
                level.side == "buy"
                    && level.buy_price < self.current_price
                    && !level.buy_filled
                    && level.buy_order_id.is_none()
                    && i.saturating_sub(current_level_idx) <= self.initial_order_range
            })
            .map(|(_, level)| level.clone())
            .collect();

        for level in &buy_init_levels {
            self.place_buy_order(level).await;
        }

        /* 挂 sell 层初始卖单 */
        let sell_init_levels: Vec<GridLevel> = self.levels.iter().enumerate()
            .filter(|(i, level)| {
                level.side == "sell"
                    && level.sell_price > self.current_price
                    && !level.sell_filled
                    && level.sell_order_id.is_none()
                    && i.saturating_sub(current_level_idx) <= self.initial_order_range
            })
            .map(|(_, level)| level.clone())
            .collect();

        for level in &sell_init_levels {
            self.place_sell_order(level).await;
        }

        /* 为已有持仓的层级挂平仓单 */
        let close_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|level| level.hold_quantity.abs() > 0.0
                && ((level.side == "buy" && level.hold_quantity > 0.0 && level.sell_order_id.is_none())
                    || (level.side == "sell" && level.hold_quantity < 0.0 && level.buy_order_id.is_none())))
            .cloned()
            .collect();

        for level in &close_levels {
            if level.side == "buy" {
                self.place_sell_order(level).await;
            } else {
                self.place_buy_order(level).await;
            }
        }

        info!(bot_id = %self.bot.id, current_level = current_level_idx, "Initial orders placed");
    }

    /// 价格 tick 处理
    ///
    /// 检查各层级是否需要挂新单或平仓单
    pub(crate) async fn on_price_tick(&mut self) {
        if self.current_price <= 0.0 {
            return;
        }

        /* buy 层开仓：价格低于买入价且未挂单 */
        let buy_open_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|level| {
                level.side == "buy"
                    && self.current_price < level.buy_price
                    && !level.buy_filled
                    && level.buy_order_id.is_none()
            })
            .cloned()
            .collect();

        for level in &buy_open_levels {
            self.place_buy_order(level).await;
        }

        /* sell 层开仓：价格高于卖出价且未挂单 */
        let sell_open_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|level| {
                level.side == "sell"
                    && level.sell_price > self.current_price
                    && !level.sell_filled
                    && level.sell_order_id.is_none()
            })
            .cloned()
            .collect();

        for level in &sell_open_levels {
            self.place_sell_order(level).await;
        }

        /* buy 层平仓：持仓为正且价格达到卖出价 */
        let buy_close_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|level| {
                level.side == "buy"
                    && level.hold_quantity > 0.0
                    && self.current_price >= level.sell_price
                    && level.sell_order_id.is_none()
            })
            .cloned()
            .collect();

        for level in &buy_close_levels {
            self.place_sell_order(level).await;
        }

        /* sell 层平仓：持仓为负且价格达到买入价 */
        let sell_close_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|level| {
                level.side == "sell"
                    && level.hold_quantity < 0.0
                    && self.current_price <= level.buy_price
                    && level.buy_order_id.is_none()
            })
            .cloned()
            .collect();

        for level in &sell_close_levels {
            self.place_buy_order(level).await;
        }

        self.broadcast_state();
    }

    /// 挂买单
    ///
    /// sell 层的买单为平仓单（reduce_only=true），buy 层的买单为开仓单
    pub(crate) async fn place_buy_order(&mut self, level: &GridLevel) {
        let key = (level.level as usize, "buy".to_string());
        if self.pending_orders.contains_key(&key) {
            return;
        }
        let (amount, reduce_only) = if level.side == "sell" {
            (level.hold_quantity.abs().min(level.quantity), true)
        } else {
            (level.quantity, false)
        };
        let client_order_id = Some(format!("grid:{}:{}:buy", self.bot.id, level.level));
        let cmd = OrderCommand::PlaceOrder {
            symbol: self.bot.symbol.clone(),
            side: OrderSide::Buy,
            amount,
            price: Some(level.buy_price),
            reduce_only,
            client_order_id,
        };
        if let Err(e) = self.order_executor.send_command(cmd).await {
            tracing::error!(bot_id = %self.bot.id, level = level.level, error = %e, "Failed to send buy order");
        } else {
            self.pending_orders.insert(key, true);
        }
    }

    /// 挂卖单
    ///
    /// buy 层的卖单为平仓单（reduce_only=true），sell 层的卖单为开仓单
    pub(crate) async fn place_sell_order(&mut self, level: &GridLevel) {
        let key = (level.level as usize, "sell".to_string());
        if self.pending_orders.contains_key(&key) {
            return;
        }
        let (amount, reduce_only) = if level.side == "sell" {
            (level.quantity, false)
        } else {
            (level.hold_quantity.min(level.quantity), true)
        };
        let client_order_id = Some(format!("grid:{}:{}:sell", self.bot.id, level.level));
        let cmd = OrderCommand::PlaceOrder {
            symbol: self.bot.symbol.clone(),
            side: OrderSide::Sell,
            amount,
            price: Some(level.sell_price),
            reduce_only,
            client_order_id,
        };
        if let Err(e) = self.order_executor.send_command(cmd).await {
            tracing::error!(bot_id = %self.bot.id, level = level.level, error = %e, "Failed to send sell order");
        } else {
            self.pending_orders.insert(key, true);
        }
    }
}
