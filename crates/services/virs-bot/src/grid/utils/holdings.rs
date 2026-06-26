//! Grid holdings calculation utilities.

/// Calculate total holdings value and unrealized PnL across all grid levels.
pub fn calc_holdings_summary(
    levels: &[(i32, f64, f64, f64)], // (level, avg_buy_price, hold_quantity, current_price)
    leverage: i32,
) -> (f64, f64, f64) {
    let mut total_hold_value = 0.0;
    let mut total_cost = 0.0;
    let mut total_unrealized_pnl = 0.0;

    for &(_level, avg_buy_price, hold_qty, current_price) in levels {
        if hold_qty > 0.0 && avg_buy_price > 0.0 {
            let hold_value = hold_qty * current_price;
            let cost = hold_qty * avg_buy_price;
            total_hold_value += hold_value;
            total_cost += cost;
            total_unrealized_pnl += (current_price - avg_buy_price) * hold_qty * leverage as f64;
        }
    }

    (total_hold_value, total_cost, total_unrealized_pnl)
}

/// Calculate margin usage rate.
pub fn calc_margin_usage(used_margin: f64, total_balance: f64) -> f64 {
    if total_balance <= 0.0 {
        return 0.0;
    }
    used_margin / total_balance * 100.0
}

/// Check if total position exceeds safety threshold (30% of investment).
pub fn is_position_over_limit(total_hold_value: f64, total_investment: f64) -> bool {
    if total_investment <= 0.0 {
        return false;
    }
    total_hold_value > total_investment * 0.3
}

/// Check if unrealized loss exceeds safety threshold (15% of investment).
pub fn is_unrealized_loss_over_limit(unrealized_pnl: f64, total_investment: f64) -> bool {
    if total_investment <= 0.0 {
        return false;
    }
    unrealized_pnl < -total_investment * 0.15
}
