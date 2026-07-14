pub mod prompt;

#[cfg(test)]
mod prompt_tests;

use crate::grid::ports::GridBotConfig;
use crate::grid::types::GridLevel;


pub fn calculate_levels(bot: &GridBotConfig, current_price: f64) -> Vec<GridLevel> {
    let effective_price = if current_price > 0.0 {
        current_price
    } else {
        (bot.upper_price + bot.lower_price) / 2.0
    };
    let width = bot.upper_price - bot.lower_price;
    if width <= 0.0 || bot.grid_count <= 0 {
        return vec![];
    }

    let step = width / (bot.grid_count + 1) as f64;
    let quantity = if bot.quantity_per_grid > 0.0 && effective_price > 0.0 {
        bot.quantity_per_grid / effective_price
    } else {
        0.0
    };

    (0..bot.grid_count)
        .map(|i| {
            let price = bot.lower_price + step * (i + 1) as f64;
            let side = if price < effective_price {
                "buy"
            } else {
                "sell"
            }
            .to_string();
            let grid_profit_mult = 1.0 + bot.grid_profit_pct / 100.0;
            GridLevel {
                level: i,
                price,
                side,
                buy_price: price,
                sell_price: price * grid_profit_mult,
                quantity,
                buy_order_id: None,
                sell_order_id: None,
                buy_filled: false,
                sell_filled: false,
                hold_quantity: 0.0,
                avg_buy_price: 0.0,
                last_fill_price: None,
                trade_id: None,
            }
        })
        .collect()
}
