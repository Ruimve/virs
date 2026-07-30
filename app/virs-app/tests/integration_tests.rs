use chrono::Utc;
use uuid::Uuid;
use virs_app::adapters::auto_store::bot_to_config as auto_bot_to_config;
use virs_app::adapters::llm_resolver::resolve_llm_provider;
use virs_app::adapters::market_data::candle_to_kline;
use virs_app::adapters::order_executor::convert_pe_event;
use virs_ccxt::ws_types::Candle;
use virs_models::AutoBot;
use virs_types::bot::OrderEvent;
use virs_types::enums::{OrderType, PositionSide, Side, TradeType};
use virs_types::position::{EngineEvent, Trade};
use virs_types::{CcxtOrder, CcxtOrderStatus, ExecutionType};

fn make_auto_bot() -> AutoBot {
    AutoBot {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        name: "auto-int".to_string(),
        symbol: "ETH/USDT".to_string(),
        exchange: "okx".to_string(),
        paper_mode: true,
        status: "running".to_string(),
        leverage: 10,
        max_position_pct: 80.0,
        decide_interval_secs: 120,
        initial_capital: 10000.0,
        position_id_long: Some(Uuid::new_v4()),
        position_id_short: Some(Uuid::new_v4()),
        market_regime: Some("volatile".to_string()),
        ai_analysis: Some("neutral".to_string()),
        system_prompt: Some("sys".to_string()),
        user_prompt: Some("usr".to_string()),
        total_pnl: 250.0,
        total_trades: 8,
        win_trades: 5,
        loss_trades: 3,
        last_decided_at: Some(Utc::now()),
        strategy_file: None,
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

fn make_order(side: Side) -> CcxtOrder {
    CcxtOrder {
        order_id: 999,
        client_order_id: "CL999".to_string(),
        symbol: "BTC/USDT".to_string(),
        side,
        order_type: OrderType::Limit,
        position_side: PositionSide::Long,
        original_order_type: Some("LIMIT".to_string()),
        status: CcxtOrderStatus::Filled,
        execution_type: ExecutionType::Trade,
        orig_qty: "2.0".to_string(),
        original_price: "42000.0".to_string(),
        avg_fill_price: Some("42100.0".to_string()),
        filled_qty: "2.0".to_string(),
        last_fill_qty: "2.0".to_string(),
        last_fill_price: "42100.0".to_string(),
        stop_price: None,
        commission: "0.2".to_string(),
        commission_asset: "USDT".to_string(),
        realized_pnl: Some("0".to_string()),
        reduce_only: false,
        is_maker: false,
        close_position: None,
        time_in_force: "GTC".to_string(),
        working_type: Some("CONTRACT_PRICE".to_string()),
        bids_notional: None,
        ask_notional: None,
        activation_price: None,
        callback_rate: None,
        price_protection: Some(false),
        stp_mode: None,
        price_match_mode: None,
        gtd_auto_cancel_time: None,
        expiry_reason: None,
        si: Some(0),
        ss: Some(0),
        trade_time: 0,
        trade_id: 0,
        modify_id: None,
        envelope_event_type: "ORDER_TRADE_UPDATE".to_string(),
        envelope_event_time: 0,
        envelope_transaction_time: 0,
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

#[test]
fn int_1_2_auto_bot_to_config_then_compare() {
    let bot = make_auto_bot();
    let config = auto_bot_to_config(&bot);
    assert_eq!(config.id, bot.id);
    assert_eq!(config.name, bot.name);
    assert_eq!(config.symbol, bot.symbol);
    assert_eq!(config.exchange, bot.exchange);
    assert_eq!(config.leverage, bot.leverage);
    assert_eq!(config.max_position_pct, bot.max_position_pct);
    assert_eq!(config.decide_interval_secs, bot.decide_interval_secs);
    assert_eq!(config.position_id_long, bot.position_id_long);
    assert_eq!(config.position_id_short, bot.position_id_short);
    assert_eq!(config.market_regime, bot.market_regime);
    assert_eq!(config.total_pnl, bot.total_pnl);
    assert_eq!(config.total_trades, bot.total_trades);
    assert_eq!(config.win_trades, bot.win_trades);
    assert_eq!(config.loss_trades, bot.loss_trades);
}

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
fn int_3_1_llm_resolve_priority_chain() {
    let creds = vec![
        ("openai".to_string(), "oai-key".to_string(), None),
        ("deepseek".to_string(), "ds-key".to_string(), None),
        ("openrouter".to_string(), "or-key".to_string(), None),
    ];
    let (key, _, _, provider) = resolve_llm_provider(&creds).unwrap();
    assert_eq!(key, "ds-key");
    assert_eq!(provider, "deepseek");

    let creds2 = vec![
        ("openai".to_string(), "oai-key".to_string(), None),
        ("openrouter".to_string(), "or-key".to_string(), None),
    ];
    let (key2, _, _, provider2) = resolve_llm_provider(&creds2).unwrap();
    assert_eq!(key2, "oai-key");
    assert_eq!(provider2, "openai");

    let creds3 = vec![("openrouter".to_string(), "or-key".to_string(), None)];
    let (key3, _, _, provider3) = resolve_llm_provider(&creds3).unwrap();
    assert_eq!(key3, "or-key");
    assert_eq!(provider3, "openrouter");
}

#[test]
fn int_3_2_llm_resolve_user_model_override() {
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
    let (key, _, model, provider) = resolve_llm_provider(&creds).unwrap();
    assert_eq!(key, "user-ds");
    assert_eq!(model, "deepseek-reasoner");
    assert_eq!(provider, "deepseek");
}

#[test]
fn int_4_1_convert_event_order_placed_filled() {
    let order = make_order(Side::Buy);
    let expected_id = Uuid::from_u128(order.order_id as u128);
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
            assert_eq!(order.id, expected_id);
            assert_eq!(order.side, Side::Buy);
        }
        _ => panic!("Expected OrderFilled"),
    }
}

#[test]
fn int_4_2_convert_event_canceled_failed() {
    let order = make_order(Side::Sell);
    let expected_id = Uuid::from_u128(order.order_id as u128);
    let event1 = EngineEvent::OrderCanceled { order };
    let result1 = convert_pe_event(event1);
    match result1.unwrap() {
        OrderEvent::OrderCanceled {
            order_id: id,
            symbol,
        } => {
            assert_eq!(id, expected_id);
            assert_eq!(symbol.as_deref(), Some("BTC/USDT"));
        }
        _ => panic!("Expected OrderCanceled"),
    }

    let event2 = EngineEvent::OrderFailed {
        client_order_id: "CL999".to_string(),
        reason: "timeout".to_string(),
    };
    let result2 = convert_pe_event(event2);
    match result2.unwrap() {
        OrderEvent::OrderFailed {
            order_id: _,
            client_order_id,
            reason,
        } => {
            assert_eq!(reason, "timeout");
            assert_eq!(client_order_id.as_deref(), Some("CL999"));
        }
        _ => panic!("Expected OrderFailed"),
    }
}

#[test]
fn int_6_2_llm_resolve_default_models() {
    let creds_ds = vec![("deepseek".to_string(), "key".to_string(), None)];
    let (_, _, model, _) = resolve_llm_provider(&creds_ds).unwrap();
    assert_eq!(model, "deepseek-chat");

    let creds_oai = vec![("openai".to_string(), "key".to_string(), None)];
    let (_, _, model, _) = resolve_llm_provider(&creds_oai).unwrap();
    assert_eq!(model, "gpt-4o");

    let creds_or = vec![("openrouter".to_string(), "key".to_string(), None)];
    let (_, _, model, _) = resolve_llm_provider(&creds_or).unwrap();
    assert_eq!(model, "deepseek/deepseek-chat");
}
