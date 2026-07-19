use crate::adapters::grid_store::bot_to_config;
use chrono::Utc;
use uuid::Uuid;
use virs_models::GridBot;
use virs_types::enums::StrategyStatus;

fn make_bot() -> GridBot {
    GridBot {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        name: "test-bot".to_string(),
        symbol: "BTC/USDT".to_string(),
        exchange: "binance".to_string(),
        paper_mode: false,
        status: StrategyStatus::Running,
        upper_price: 110.0,
        lower_price: 90.0,
        grid_count: 10,
        grid_profit_pct: 0.5,
        quantity_per_grid: 100.0,
        leverage: 5,
        initial_capital: 10000.0,
        market_regime: None,
        ai_analysis: None,
        grid_levels_json: None,
        system_prompt: None,
        user_prompt: None,
        dynamic_adjust: false,
        adjust_interval_secs: 300,
        last_adjusted_at: None,
        strategy_file: None,
        total_pnl: 0.0,
        unrealized_pnl: 0.0,
        total_trades: 0,
        grid_filled_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        started_at: None,
        stopped_at: None,
    }
}

#[test]
fn g1_1_bot_to_config_basic() {
    let bot = make_bot();
    let config = bot_to_config(&bot);
    assert_eq!(config.name, "test-bot");
    assert_eq!(config.symbol, "BTC/USDT");
    assert_eq!(config.exchange, "binance");
    assert!(!config.paper_mode);
}

#[test]
fn g1_2_bot_to_config_optional_fields() {
    let bot = make_bot();
    let config = bot_to_config(&bot);
    assert!(config.market_regime.is_none());
    assert!(config.grid_levels_json.is_none());
    assert!(config.system_prompt.is_none());
    assert!(config.last_adjusted_at.is_none());
}

#[test]
fn g1_3_bot_to_config_some_fields() {
    let mut bot = make_bot();
    bot.market_regime = Some("ranging".to_string());
    bot.system_prompt = Some("You are a grid bot".to_string());
    let config = bot_to_config(&bot);
    assert_eq!(config.market_regime.as_deref(), Some("ranging"));
    assert_eq!(config.system_prompt.as_deref(), Some("You are a grid bot"));
}

#[test]
fn g1_4_bot_to_config_numeric() {
    let bot = make_bot();
    let config = bot_to_config(&bot);
    assert!((config.upper_price - 110.0).abs() < 1e-10);
    assert!((config.lower_price - 90.0).abs() < 1e-10);
    assert_eq!(config.grid_count, 10);
    assert!((config.grid_profit_pct - 0.5).abs() < 1e-10);
    assert!((config.quantity_per_grid - 100.0).abs() < 1e-10);
    assert_eq!(config.leverage, 5);
    assert_eq!(config.adjust_interval_secs, 300);
}

#[test]
fn g1_5_bot_to_config_id_preserved() {
    let bot = make_bot();
    let config = bot_to_config(&bot);
    assert_eq!(config.id, bot.id);
    assert_eq!(config.user_id, bot.user_id);
}
