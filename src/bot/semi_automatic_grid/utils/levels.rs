use crate::bot::semi_automatic_grid::ports::GridBotConfig;
use crate::bot::semi_automatic_grid::types::GridLevel;

struct LevelParams {
    grid_spacing: f64,
    profit_factor: f64,
}

fn extract_level_params(bot: &GridBotConfig) -> Option<LevelParams> {
    if bot.grid_count <= 0 || bot.upper_price <= 0.0 || bot.lower_price <= 0.0 || bot.upper_price <= bot.lower_price {
        return None;
    }
    Some(LevelParams {
        grid_spacing: (bot.upper_price - bot.lower_price) / bot.grid_count as f64,
        profit_factor: 1.0 + bot.grid_profit_pct / 100.0,
    })
}

fn determine_level_side(price: f64, current_price: f64) -> String {
    if price < current_price { "buy".to_string() } else { "sell".to_string() }
}

fn compute_buy_sell_prices(side: &str, price: f64, profit_factor: f64) -> (f64, f64) {
    if side == "buy" {
        (price, price * profit_factor)
    } else {
        (price / profit_factor, price)
    }
}

fn compute_quantity(price: f64, quantity_per_grid: f64) -> f64 {
    if price > 0.0 { quantity_per_grid / price } else { 0.0 }
}

pub fn calculate_levels(bot: &GridBotConfig, current_price: f64) -> Vec<GridLevel> {
    let params = match extract_level_params(bot) {
        Some(p) => p,
        None => return vec![],
    };

    let side_threshold = if current_price > 0.0 {
        current_price
    } else {
        (bot.upper_price + bot.lower_price) / 2.0
    };

    (0..bot.grid_count)
        .map(|i| {
            let price = bot.lower_price + params.grid_spacing * (i as f64 + 0.5);
            let side = determine_level_side(price, side_threshold);
            let (buy_price, sell_price) = compute_buy_sell_prices(&side, price, params.profit_factor);
            let quantity = compute_quantity(price, bot.quantity_per_grid);

            GridLevel {
                level: i,
                price,
                side,
                buy_price,
                sell_price,
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

pub fn compute_grid_spacing(upper_price: f64, lower_price: f64, grid_count: i32) -> f64 {
    if grid_count > 1 {
        (upper_price - lower_price) / grid_count as f64
    } else {
        0.0
    }
}

pub fn compute_profit_factor(grid_profit_pct: f64) -> f64 {
    1.0 + grid_profit_pct / 100.0
}

pub fn compute_mid_price(upper_price: f64, lower_price: f64) -> f64 {
    (upper_price + lower_price) / 2.0
}
