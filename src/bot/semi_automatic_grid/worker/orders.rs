use tracing::{info, warn};

use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::types::GridLevel;
use crate::bot::semi_automatic_grid::worker::GridWorker;

/** 挂单方向枚举

用于参数化 place_order 方法，避免 buy/sell 两个方法的大面积重复 */
pub enum OrderDir {
    Buy,
    Sell,
}

impl GridWorker {
    /** 挂初始订单

    在当前价格附近 ±initial_order_range 层范围内挂买卖单，
    同时为已有持仓的层级挂平仓单 */
    pub(crate) async fn place_initial_orders(&mut self) {
        if self.current_price <= 0.0 {
            warn!(bot_id = %self.bot.id, "No current price, skipping initial orders");
            return;
        }

        let current_level_idx = self.find_level_by_price(self.current_price);
        let range = self.initial_order_range;

        /* buy 层初始买单：价格低于买入价、未成交、未挂单、在范围内 */
        let levels: Vec<GridLevel> = self.levels.iter().enumerate()
            .filter(|(i, l)| {
                l.side == "buy"
                    && l.buy_price < self.current_price
                    && !l.buy_filled
                    && l.buy_order_id.is_none()
                    && (*i as i32 - current_level_idx as i32).abs() <= range as i32
            })
            .map(|(_, l)| l.clone())
            .collect();
        self.place_orders_for_levels(&levels, OrderDir::Buy).await;

        /* sell 层初始卖单：价格高于卖出价、未成交、未挂单、在范围内 */
        let levels: Vec<GridLevel> = self.levels.iter().enumerate()
            .filter(|(i, l)| {
                l.side == "sell"
                    && l.sell_price > self.current_price
                    && !l.sell_filled
                    && l.sell_order_id.is_none()
                    && (*i as i32 - current_level_idx as i32).abs() <= range as i32
            })
            .map(|(_, l)| l.clone())
            .collect();
        self.place_orders_for_levels(&levels, OrderDir::Sell).await;

        /* 为已有持仓的层级挂平仓单 */
        let close_levels: Vec<GridLevel> = self.levels.iter()
            .filter(|l| l.hold_quantity.abs() > 0.0
                && ((l.side == "buy" && l.hold_quantity > 0.0 && l.sell_order_id.is_none())
                    || (l.side == "sell" && l.hold_quantity < 0.0 && l.buy_order_id.is_none())))
            .cloned()
            .collect();

        for level in &close_levels {
            let dir = if level.side == "buy" { OrderDir::Sell } else { OrderDir::Buy };
            self.place_order(level, &dir).await;
        }

        info!(bot_id = %self.bot.id, current_level = current_level_idx, "Initial orders placed");
    }

    /** 价格 tick 处理

    检查各层级是否需要挂新单或平仓单 */
    pub(crate) async fn on_price_tick(&mut self) {
        if self.current_price <= 0.0 {
            return;
        }

        /* buy 层开仓：价格低于买入价且未挂单 */
        let levels = self.filter_levels(|l| {
            l.side == "buy"
                && self.current_price < l.buy_price
                && !l.buy_filled
                && l.buy_order_id.is_none()
        });
        self.place_orders_for_levels(&levels, OrderDir::Buy).await;

        /* sell 层开仓：价格高于卖出价且未挂单 */
        let levels = self.filter_levels(|l| {
            l.side == "sell"
                && l.sell_price > self.current_price
                && !l.sell_filled
                && l.sell_order_id.is_none()
        });
        self.place_orders_for_levels(&levels, OrderDir::Sell).await;

        /* buy 层平仓：持仓为正且价格达到卖出价 */
        let levels = self.filter_levels(|l| {
            l.side == "buy"
                && l.hold_quantity > 0.0
                && self.current_price >= l.sell_price
                && l.sell_order_id.is_none()
        });
        self.place_orders_for_levels(&levels, OrderDir::Sell).await;

        /* sell 层平仓：持仓为负且价格达到买入价 */
        let levels = self.filter_levels(|l| {
            l.side == "sell"
                && l.hold_quantity < 0.0
                && self.current_price <= l.buy_price
                && l.buy_order_id.is_none()
        });
        self.place_orders_for_levels(&levels, OrderDir::Buy).await;

        self.broadcast_state();
    }

    /** 按条件过滤层级并克隆 */
    fn filter_levels(&self, predicate: impl Fn(&GridLevel) -> bool) -> Vec<GridLevel> {
        self.levels.iter().filter(|l| predicate(l)).cloned().collect()
    }

    /** 对一组层级批量挂单 */
    async fn place_orders_for_levels(&mut self, levels: &[GridLevel], dir: OrderDir) {
        for level in levels {
            self.place_order(level, &dir).await;
        }
    }

    /** 通用挂单方法

    根据 OrderDir 决定挂买/卖单，自动计算 amount 和 reduce_only：
    - buy 层的买单为开仓单，卖单为平仓单
    - sell 层的卖单为开仓单，买单为平仓单 */
    pub(crate) async fn place_order(&mut self, level: &GridLevel, dir: &OrderDir) {
        let (side_str, price, key_side) = match dir {
            OrderDir::Buy => (OrderSide::Buy, level.buy_price, "buy"),
            OrderDir::Sell => (OrderSide::Sell, level.sell_price, "sell"),
        };

        let key = (level.level as usize, key_side.to_string());
        if self.pending_orders.contains(&key) {
            return;
        }

        let (amount, reduce_only, position_side) = match (dir, level.side.as_str()) {
            (OrderDir::Buy, "sell") => (level.hold_quantity.abs().min(level.quantity), true, Some(PositionSide::Short)),
            (OrderDir::Buy, _) => (level.quantity, false, Some(PositionSide::Long)),
            (OrderDir::Sell, "sell") => (level.quantity, false, Some(PositionSide::Short)),
            (OrderDir::Sell, _) => (level.hold_quantity.min(level.quantity), true, Some(PositionSide::Long)),
        };

        let client_order_id = Some(format!("grid:{}:{}:{}", self.bot.id, level.level, key_side));
        let cmd = OrderCommand::PlaceOrder {
            symbol: self.bot.symbol.clone(),
            side: side_str,
            amount,
            price: Some(price),
            reduce_only,
            position_side,
            client_order_id,
        };

        if let Err(e) = self.order_executor.send_command(cmd).await {
            tracing::error!(bot_id = %self.bot.id, level = level.level, side = key_side, error = %e, "Failed to send order");
        } else {
            self.pending_orders.insert(key);
        }
    }

    /** 挂买单（兼容旧接口，委托给 place_order） */
    pub(crate) async fn place_buy_order(&mut self, level: &GridLevel) {
        self.place_order(level, &OrderDir::Buy).await;
    }

    /** 挂卖单（兼容旧接口，委托给 place_order） */
    pub(crate) async fn place_sell_order(&mut self, level: &GridLevel) {
        self.place_order(level, &OrderDir::Sell).await;
    }
}
