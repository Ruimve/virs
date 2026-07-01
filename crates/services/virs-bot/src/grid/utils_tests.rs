//! Unit tests for grid/utils/mod.rs

use crate::grid::utils::calculate_levels;
use uuid::Uuid;
use virs_types::grid_port::GridBotConfig;

fn make_bot(upper: f64, lower: f64, count: i32, profit_pct: f64, qty: f64) -> GridBotConfig {
    GridBotConfig {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        name: "test".to_string(),
        symbol: "BTC/USDT".to_string(),
        exchange: "binance".to_string(),
        market_type: "futures".to_string(),
        paper_mode: false,
        grid_count: count,
        upper_price: upper,
        lower_price: lower,
        grid_profit_pct: profit_pct,
        quantity_per_grid: qty,
        leverage: 5,
        dynamic_adjust: false,
        adjust_interval_secs: 300,
        market_regime: None,
        grid_levels_json: None,
        system_prompt: None,
        last_adjusted_at: None,
    }
}

#[test]
fn u1_1_calculate_levels_basic() {
    let bot = make_bot(110.0, 90.0, 10, 0.5, 100.0);
    let levels = calculate_levels(&bot, 100.0);
    assert_eq!(levels.len(), 10);
    // step = (110 - 90) / (10 + 1) = 20/11 ≈ 1.818
    let step = (110.0 - 90.0) / 11.0;
    let first_price = 90.0 + step;
    assert!((levels[0].price - first_price).abs() < 1e-10);
    assert_eq!(levels[0].level, 0);
}

#[test]
fn u1_2_calculate_levels_zero_width() {
    let bot = make_bot(100.0, 100.0, 10, 0.5, 100.0);
    let levels = calculate_levels(&bot, 100.0);
    assert!(levels.is_empty());
}

#[test]
fn u1_3_calculate_levels_zero_count() {
    let bot = make_bot(110.0, 90.0, 0, 0.5, 100.0);
    let levels = calculate_levels(&bot, 100.0);
    assert!(levels.is_empty());
}

#[test]
fn u1_4_calculate_levels_side_assignment() {
    let bot = make_bot(110.0, 90.0, 10, 0.5, 100.0);
    let levels = calculate_levels(&bot, 100.0);
    // Prices below 100.0 → "buy", at or above → "sell"
    for level in &levels {
        if level.price < 100.0 {
            assert_eq!(level.side, "buy");
        } else {
            assert_eq!(level.side, "sell");
        }
    }
}

#[test]
fn u1_5_calculate_levels_sell_price() {
    let bot = make_bot(110.0, 90.0, 5, 1.0, 100.0);
    let levels = calculate_levels(&bot, 100.0);
    let grid_profit_mult = 1.0 + 1.0 / 100.0; // 1.01
    for level in &levels {
        assert!((level.sell_price - level.price * grid_profit_mult).abs() < 1e-10);
    }
}

#[test]
fn u1_6_calculate_levels_uses_current_price_for_qty() {
    let bot = make_bot(110.0, 90.0, 5, 0.5, 100.0);
    let levels = calculate_levels(&bot, 100.0);
    // quantity = qty_per_grid / current_price = 100 / 100 = 1.0
    for level in &levels {
        assert!((level.quantity - 1.0).abs() < 1e-10);
    }
}
