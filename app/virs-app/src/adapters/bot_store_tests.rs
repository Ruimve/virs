use crate::adapters::bot_store::bot_to_config;
use chrono::Utc;
use uuid::Uuid;
use virs_type::Bot;

fn make_bot() -> Bot {
    Bot {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        name: "chat-test".to_string(),
        symbol: "ETHUSDT".to_string(),
        exchange: "okx".to_string(),
        paper_mode: true,
        status: "running".to_string(),
        bot_type: "chat".to_string(),
        leverage: 10,
        max_position_pct: 80.0,
        decide_interval_secs: 60,
        initial_capital: 5000.0,
        position_id_long: None,
        position_id_short: None,
        market_regime: None,
        ai_analysis: None,
        system_prompt: None,
        user_prompt: None,
        total_pnl: 0.0,
        total_trades: 0,
        win_trades: 0,
        loss_trades: 0,
        last_decided_at: None,
        strategy_file: None,
        auto_optimize_enabled: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        started_at: None,
        stopped_at: None,
    }
}

#[test]
fn a1_1_bot_to_config_basic() {
    let bot = make_bot();
    let config = bot_to_config(&bot);
    assert_eq!(config.name, "chat-test");
    assert_eq!(config.symbol, "ETHUSDT");
    assert_eq!(config.exchange, "okx");
    assert!(config.paper_mode);
    assert_eq!(config.leverage, 10);
}

#[test]
fn a1_3_bot_to_config_optional_fields() {
    let bot = make_bot();
    let config = bot_to_config(&bot);
    assert!(config.position_id_long.is_none());
    assert!(config.position_id_short.is_none());
    assert!(config.market_regime.is_none());
    assert!(config.ai_analysis.is_none());
    assert!(config.system_prompt.is_none());
    assert!(config.user_prompt.is_none());
    assert!(config.last_decided_at.is_none());
}

#[test]
fn a1_4_bot_to_config_stats() {
    let mut bot = make_bot();
    bot.total_pnl = 123.45;
    bot.total_trades = 10;
    bot.win_trades = 7;
    bot.loss_trades = 3;
    let config = bot_to_config(&bot);
    assert!((config.total_pnl - 123.45).abs() < 1e-10);
    assert_eq!(config.total_trades, 10);
    assert_eq!(config.win_trades, 7);
    assert_eq!(config.loss_trades, 3);
}

#[test]
fn a1_5_bot_to_config_id_preserved() {
    let bot = make_bot();
    let config = bot_to_config(&bot);
    assert_eq!(config.id, bot.id);
    assert_eq!(config.user_id, bot.user_id);
}
