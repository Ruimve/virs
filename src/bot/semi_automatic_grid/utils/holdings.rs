use crate::bot::semi_automatic_grid::types::GridLevel;

/** 将一笔成交应用到网格层级的持仓状态

根据 level.side 和成交方向（trade_side）更新 hold_quantity 和 avg_buy_price：
- buy 层 + buy 成交：加权均价，增加持仓
- buy 层 + sell 成交：减少持仓
- sell 层 + sell 成交：加权均价，增加空头持仓
- sell 层 + buy 成交：减少空头持仓 */
pub fn apply_fill_to_level(level: &mut GridLevel, trade_side: &str, price: f64, quantity: f64) {
    let is_buy = trade_side == "buy";
    if level.side == "buy" {
        if is_buy {
            let old_total = level.avg_buy_price * level.hold_quantity;
            let new_total = old_total + price * quantity;
            level.hold_quantity += quantity;
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
            level.hold_quantity = (level.hold_quantity - quantity).max(0.0);
            level.last_fill_price = Some(price);
        }
    } else {
        if !is_buy {
            let old_total = level.avg_buy_price * level.hold_quantity.abs();
            let new_total = old_total + price * quantity;
            level.hold_quantity -= quantity;
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
            level.hold_quantity = (level.hold_quantity + quantity).min(0.0);
            level.last_fill_price = Some(price);
        }
    }
}

/** 计算平仓成交的已实现盈亏

仅在平仓方向成交时计算（buy层卖出、sell层买入），
开仓方向成交返回 0.0 */
pub fn calculate_fill_pnl(
    level_side: &str,
    trade_side: &str,
    entry_price: f64,
    fill_price: f64,
    quantity: f64,
) -> f64 {
    let is_close = if level_side == "buy" {
        trade_side == "sell"
    } else {
        trade_side == "buy"
    };

    if !is_close || entry_price <= 0.0 {
        return 0.0;
    }

    if level_side == "buy" {
        fill_price * quantity - entry_price * quantity
    } else {
        entry_price * quantity - fill_price * quantity
    }
}

/** 判断成交方向是否为平仓操作 */
pub fn is_close_trade(level_side: &str, trade_side: &str) -> bool {
    if level_side == "buy" {
        trade_side == "sell"
    } else {
        trade_side == "buy"
    }
}
