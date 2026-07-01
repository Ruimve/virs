//! Integration tests for virs-app adapters — cross-module chain tests.

use chrono::Utc;
use uuid::Uuid;
use virs_app::adapters::auto_store::bot_to_config as auto_bot_to_config;
use virs_app::adapters::grid_store::bot_to_config as grid_bot_to_config;
use virs_app::adapters::llm_resolver::resolve_llm_provider;
use virs_app::adapters::market_data::candle_to_kline;
use virs_app::adapters::order_executor::convert_pe_event;
use virs_app::adapters::utils::{derive_open_side, sanitize_pnl_pct};
use virs_config::AiConfig;
use virs_market::Candle;
use virs_models::AutoBot;
use virs_models::GridBot;
use virs_types::auto_port::AutoMarketType;
use virs_types::bot::{OrderEvent, OrderSide};
use virs_types::enums::{OrderStatus, OrderType, Side, StrategyStatus, TradeType};
use virs_types::position::{EngineEvent, PositionOrder, Trade};

// ── helpers ───────────────────────────────────────────────

fn make_grid_bot() -> GridBot {
    GridBot {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        name: "grid-int".to_string(),
        symbol: "BTC/USDT".to_string(),
        exchange: "binance".to_string(),
        market_type: "futures".to_string(),
        paper_mode: false,
        status: StrategyStatus::Running,
        upper_price: 120.0,
        lower_price: 80.0,
        grid_count: 20,
        grid_profit_pct: 0.8,
        quantity_per_grid: 50.0,
        leverage: 5,
        initial_capital: 20000.0,
        market_regime: Some("trending".to_string()),
        ai_analysis: Some("bullish".to_string()),
        grid_levels_json: Some(serde_json::json!([{"price": 100}])),
        system_prompt: Some("system".to_string()),
        user_prompt: Some("user".to_string()),
        dynamic_adjust: true,
        adjust_interval_secs: 600,
        last_adjusted_at: Some(Utc::now()),
        total_pnl: 500.0,
        unrealized_pnl: 100.0,
        total_trades: 30,
        grid_filled_count: 15,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        started_at: Some(Utc::now()),
        stopped_at: None,
    }
}

fn make_auto_bot() -> AutoBot {
    AutoBot {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        name: "auto-int".to_string(),
        symbol: "ETH/USDT".to_string(),
        exchange: "okx".to_string(),
        market_type: "perpetual".to_string(),
        paper_mode: true,
        status: "running".to_string(),
        leverage: 10,
        max_position_pct: 80.0,
        decide_interval_secs: 120,
        initial_capital: 10000.0,
        position_id: Some(Uuid::new_v4()),
        market_regime: Some("volatile".to_string()),
        ai_analysis: Some("neutral".to_string()),
        system_prompt: Some("sys".to_string()),
        user_prompt: Some("usr".to_string()),
        total_pnl: 250.0,
        total_trades: 8,
        win_trades: 5,
        loss_trades: 3,
        last_decided_at: Some(Utc::now()),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        started_at: Some(Utc::now()),
        stopped_at: None,
    }
}

fn make_candle() -> Candle {
    Candle {
        open_time: 1700000000000,
        close_time: 1700000059999,
        open: 42000.0,
        high: 42500.0,
        low: 41800.0,
        close: 42300.0,
        volume: 1000.0,
        quote_volume: 42300000.0,
        trades: 500,
        closed: true,
    }
}

fn make_order(side: Side) -> PositionOrder {
    PositionOrder {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        exchange_order_id: Some("EX999".to_string()),
        client_order_id: Some("CL999".to_string()),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side,
        order_type: OrderType::Limit,
        request_price: Some(42000.0),
        fill_price: Some(42100.0),
        amount: 2.0,
        filled: 2.0,
        remaining: 0.0,
        status: OrderStatus::Filled,
        reduce_only: false,
        fee: 0.2,
        fee_currency: "USDT".to_string(),
        slippage: Some(1.0),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn make_trade() -> Trade {
    Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        price: 42100.0,
        amount: 2.0,
        fee: 0.2,
        fee_currency: "USDT".to_string(),
        pnl: 50.0,
        trade_type: TradeType::Open,
        created_at: Utc::now(),
    }
}

// ── INT-1: bot_to_config 跨模块一致性 ─────────────────────

#[test]
fn int_1_1_grid_bot_to_config_then_compare() {
    let bot = make_grid_bot();
    let config = grid_bot_to_config(&bot);
    assert_eq!(config.id, bot.id);
    assert_eq!(config.name, bot.name);
    assert_eq!(config.symbol, bot.symbol);
    assert_eq!(config.exchange, bot.exchange);
    assert_eq!(config.upper_price, bot.upper_price);
    assert_eq!(config.lower_price, bot.lower_price);
    assert_eq!(config.grid_count, bot.grid_count);
    assert_eq!(config.leverage, bot.leverage);
    assert_eq!(config.market_regime, bot.market_regime);
    assert_eq!(config.grid_levels_json, bot.grid_levels_json);
    assert_eq!(config.system_prompt, bot.system_prompt);
    assert_eq!(config.dynamic_adjust, bot.dynamic_adjust);
    assert_eq!(config.adjust_interval_secs, bot.adjust_interval_secs);
    assert_eq!(config.last_adjusted_at, bot.last_adjusted_at);
}

#[test]
fn int_1_2_auto_bot_to_config_then_compare() {
    let bot = make_auto_bot();
    let config = auto_bot_to_config(&bot);
    assert_eq!(config.id, bot.id);
    assert_eq!(config.name, bot.name);
    assert_eq!(config.symbol, bot.symbol);
    assert_eq!(config.exchange, bot.exchange);
    assert_eq!(config.market_type, AutoMarketType::Perpetual);
    assert_eq!(config.leverage, bot.leverage);
    assert_eq!(config.max_position_pct, bot.max_position_pct);
    assert_eq!(config.decide_interval_secs, bot.decide_interval_secs);
    assert_eq!(config.position_id, bot.position_id);
    assert_eq!(config.market_regime, bot.market_regime);
    assert_eq!(config.total_pnl, bot.total_pnl);
    assert_eq!(config.total_trades, bot.total_trades);
    assert_eq!(config.win_trades, bot.win_trades);
    assert_eq!(config.loss_trades, bot.loss_trades);
}

// ── INT-2: candle_to_kline + utils 链路 ───────────────────

#[test]
fn int_2_1_candle_to_kline_preserves_ohlcv() {
    let c = make_candle();
    let k = candle_to_kline(&c);
    assert_eq!(k.open, c.open);
    assert_eq!(k.high, c.high);
    assert_eq!(k.low, c.low);
    assert_eq!(k.close, c.close);
    assert_eq!(k.volume, c.volume);
    assert_eq!(k.quote_volume, c.quote_volume);
    assert_eq!(k.trades, c.trades);
    assert_eq!(k.open_time, c.open_time);
    assert_eq!(k.close_time, c.close_time);
}

#[test]
fn int_2_2_sanitize_then_derive_chain() {
    // Simulate close_trade flow: sanitize pnl_pct then derive open_side
    let raw_pnl_pct = f64::NAN;
    let close_side = "buy";
    let sanitized = sanitize_pnl_pct(raw_pnl_pct);
    let open_side = derive_open_side(close_side);
    assert_eq!(sanitized, 0.0);
    assert_eq!(open_side, "sell");
}

// ── INT-3: LLM 解析优先级链 ───────────────────────────────

#[test]
fn int_3_1_llm_resolve_priority_chain() {
    let config = AiConfig {
        deepseek_api_key: Some("cfg-ds".to_string()),
        openai_api_key: Some("cfg-oai".to_string()),
        openrouter_api_key: Some("cfg-or".to_string()),
    };
    let creds: Vec<(String, String, Option<String>)> = vec![];
    // deepseek wins
    let (key, _, _, provider) = resolve_llm_provider(&creds, &config).unwrap();
    assert_eq!(key, "cfg-ds");
    assert_eq!(provider, "deepseek");

    // Remove deepseek → openai wins
    let config2 = AiConfig {
        deepseek_api_key: None,
        openai_api_key: Some("cfg-oai".to_string()),
        openrouter_api_key: Some("cfg-or".to_string()),
    };
    let (key2, _, _, provider2) = resolve_llm_provider(&creds, &config2).unwrap();
    assert_eq!(key2, "cfg-oai");
    assert_eq!(provider2, "openai");

    // Remove openai → openrouter wins
    let config3 = AiConfig {
        deepseek_api_key: None,
        openai_api_key: None,
        openrouter_api_key: Some("cfg-or".to_string()),
    };
    let (key3, _, _, provider3) = resolve_llm_provider(&creds, &config3).unwrap();
    assert_eq!(key3, "cfg-or");
    assert_eq!(provider3, "openrouter");
}

#[test]
fn int_3_2_llm_resolve_user_overrides_config() {
    let config = AiConfig {
        deepseek_api_key: Some("cfg-ds".to_string()),
        openai_api_key: Some("cfg-oai".to_string()),
        openrouter_api_key: None,
    };
    // User has deepseek credential → overrides config deepseek key
    let creds = vec![
        (
            "deepseek".to_string(),
            "user-ds".to_string(),
            Some("deepseek-reasoner".to_string()),
        ),
        (
            "openai".to_string(),
            "user-oai".to_string(),
            Some("gpt-4o-mini".to_string()),
        ),
    ];
    let (key, _, model, provider) = resolve_llm_provider(&creds, &config).unwrap();
    assert_eq!(key, "user-ds");
    assert_eq!(model, "deepseek-reasoner");
    assert_eq!(provider, "deepseek");
}

// ── INT-4: 事件转换链 ─────────────────────────────────────

#[test]
fn int_4_1_convert_event_order_placed_filled() {
    let order = make_order(Side::Buy);
    let order_id = order.id;
    let event1 = EngineEvent::OrderPlaced {
        order: order.clone(),
    };
    let result1 = convert_pe_event(event1);
    assert!(matches!(result1, Some(OrderEvent::OrderPlaced { .. })));

    let event2 = EngineEvent::OrderFilled {
        order: order.clone(),
        trade: make_trade(),
    };
    let result2 = convert_pe_event(event2);
    match result2.unwrap() {
        OrderEvent::OrderFilled { order } => {
            assert_eq!(order.id, order_id);
            assert_eq!(order.side, OrderSide::Buy);
        }
        _ => panic!("Expected OrderFilled"),
    }
}

#[test]
fn int_4_2_convert_event_canceled_failed() {
    let order = make_order(Side::Sell);
    let order_id = order.id;
    let event1 = EngineEvent::OrderCanceled { order };
    let result1 = convert_pe_event(event1);
    match result1.unwrap() {
        OrderEvent::OrderCanceled {
            order_id: id,
            symbol,
        } => {
            assert_eq!(id, order_id);
            assert_eq!(symbol.as_deref(), Some("BTC/USDT"));
        }
        _ => panic!("Expected OrderCanceled"),
    }

    let oid = Uuid::new_v4();
    let event2 = EngineEvent::OrderFailed {
        order_id: oid,
        reason: "timeout".to_string(),
    };
    let result2 = convert_pe_event(event2);
    match result2.unwrap() {
        OrderEvent::OrderFailed { order_id, reason } => {
            assert_eq!(order_id, oid);
            assert_eq!(reason, "timeout");
        }
        _ => panic!("Expected OrderFailed"),
    }
}

// ── INT-5: utils 全覆盖 ───────────────────────────────────

#[test]
fn int_5_1_sanitize_all_pnl_cases() {
    assert_eq!(sanitize_pnl_pct(0.15), 0.15);
    assert_eq!(sanitize_pnl_pct(f64::NAN), 0.0);
    assert_eq!(sanitize_pnl_pct(0.0), 0.0);
    assert_eq!(sanitize_pnl_pct(-0.5), -0.5);
    assert_eq!(sanitize_pnl_pct(1.0), 1.0);
}

#[test]
fn int_5_2_derive_open_side_all_cases() {
    assert_eq!(derive_open_side("buy"), "sell");
    assert_eq!(derive_open_side("sell"), "buy");
    // unknown strings default to "buy"
    assert_eq!(derive_open_side("unknown"), "buy");
    assert_eq!(derive_open_side(""), "buy");
}

// ── INT-6: 跨模块一致性 ───────────────────────────────────

#[test]
fn int_6_1_grid_auto_bot_to_config_consistency() {
    // Both conversions are independent — running one doesn't affect the other
    let grid_bot = make_grid_bot();
    let auto_bot = make_auto_bot();

    let grid_config = grid_bot_to_config(&grid_bot);
    let auto_config = auto_bot_to_config(&auto_bot);

    // Grid config should not have auto fields and vice versa
    assert_eq!(grid_config.name, "grid-int");
    assert_eq!(auto_config.name, "auto-int");
    assert_ne!(grid_config.symbol, auto_config.symbol);

    // IDs are preserved independently
    assert_eq!(grid_config.id, grid_bot.id);
    assert_eq!(auto_config.id, auto_bot.id);
    assert_ne!(grid_config.id, auto_config.id);
}

#[test]
fn int_6_2_llm_resolve_default_models() {
    // deepseek default model
    let config_ds = AiConfig {
        deepseek_api_key: Some("key".to_string()),
        openai_api_key: None,
        openrouter_api_key: None,
    };
    let (_, _, model, _) = resolve_llm_provider(&[], &config_ds).unwrap();
    assert_eq!(model, "deepseek-chat");

    // openai default model
    let config_oai = AiConfig {
        deepseek_api_key: None,
        openai_api_key: Some("key".to_string()),
        openrouter_api_key: None,
    };
    let (_, _, model, _) = resolve_llm_provider(&[], &config_oai).unwrap();
    assert_eq!(model, "gpt-4o");

    // openrouter default model
    let config_or = AiConfig {
        deepseek_api_key: None,
        openai_api_key: None,
        openrouter_api_key: Some("key".to_string()),
    };
    let (_, _, model, _) = resolve_llm_provider(&[], &config_or).unwrap();
    assert_eq!(model, "deepseek/deepseek-chat");
}
