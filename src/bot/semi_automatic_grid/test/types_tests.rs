use crate::bot::semi_automatic_grid::types::*;
use uuid::Uuid;

#[test]
fn grid_event_serialization() {
    let bot_id = Uuid::new_v4();

    let event = GridEvent::BotStarted { bot_id };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("BotStarted"));
    let deserialized: GridEvent = serde_json::from_str(&json).unwrap();
    match deserialized {
        GridEvent::BotStarted { bot_id: b } => assert_eq!(b, bot_id),
        _ => panic!("Expected BotStarted"),
    }

    let event = GridEvent::BotStopped { bot_id, reason: "test".to_string() };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("BotStopped"));
    let deserialized: GridEvent = serde_json::from_str(&json).unwrap();
    match deserialized {
        GridEvent::BotStopped { bot_id: b, reason } => {
            assert_eq!(b, bot_id);
            assert_eq!(reason, "test");
        }
        _ => panic!("Expected BotStopped"),
    }

    let event = GridEvent::BotError { bot_id, error: "err".to_string() };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: GridEvent = serde_json::from_str(&json).unwrap();
    match deserialized {
        GridEvent::BotError { bot_id: b, error } => {
            assert_eq!(b, bot_id);
            assert_eq!(error, "err");
        }
        _ => panic!("Expected BotError"),
    }

    let event = GridEvent::PriceUpdate { bot_id, price: 55000.0 };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: GridEvent = serde_json::from_str(&json).unwrap();
    match deserialized {
        GridEvent::PriceUpdate { bot_id: b, price } => {
            assert_eq!(b, bot_id);
            assert!((price - 55000.0).abs() < f64::EPSILON);
        }
        _ => panic!("Expected PriceUpdate"),
    }
}

#[test]
fn grid_command_debug() {
    let bot_id = Uuid::new_v4();
    let cmd = GridCommand::StartBot { bot_id };
    let debug_str = format!("{:?}", cmd);
    assert!(debug_str.contains("StartBot"));
    assert!(debug_str.contains(&bot_id.to_string()));

    let cmd = GridCommand::StopBot { bot_id };
    assert!(format!("{:?}", cmd).contains("StopBot"));

    let cmd = GridCommand::PauseBot { bot_id };
    assert!(format!("{:?}", cmd).contains("PauseBot"));

    let cmd = GridCommand::ResumeBot { bot_id };
    assert!(format!("{:?}", cmd).contains("ResumeBot"));

    let cmd = GridCommand::DeleteBot { bot_id };
    assert!(format!("{:?}", cmd).contains("DeleteBot"));

    let cmd = GridCommand::Shutdown;
    assert!(format!("{:?}", cmd).contains("Shutdown"));
}

#[test]
fn grid_level_default_values() {
    let level = GridLevel {
        level: 0, price: 50000.0, buy_price: 50000.0, sell_price: 50250.0,
        quantity: 0.002, buy_order_id: None, sell_order_id: None,
        buy_filled: false, sell_filled: false, hold_quantity: 0.0,
    };
    assert_eq!(level.level, 0);
    assert!((level.price - 50000.0).abs() < f64::EPSILON);
    assert!(level.buy_order_id.is_none());
    assert!(level.sell_order_id.is_none());
    assert!(!level.buy_filled);
    assert!(!level.sell_filled);
    assert!((level.hold_quantity - 0.0).abs() < f64::EPSILON);
}

#[test]
fn grid_state_construction() {
    let bot_id = Uuid::new_v4();
    let state = GridState {
        bot_id, symbol: "BTCUSDT".to_string(), exchange: "binance".to_string(),
        levels: vec![], current_price: 55000.0, total_pnl: 100.0,
        total_trades: 5, grid_filled_count: 3, last_tick_at: chrono::Utc::now(),
    };
    assert_eq!(state.bot_id, bot_id);
    assert_eq!(state.symbol, "BTCUSDT");
    assert_eq!(state.exchange, "binance");
    assert!(state.levels.is_empty());
    assert!((state.current_price - 55000.0).abs() < f64::EPSILON);
    assert!((state.total_pnl - 100.0).abs() < f64::EPSILON);
    assert_eq!(state.total_trades, 5);
    assert_eq!(state.grid_filled_count, 3);
}

#[test]
fn grid_event_bot_started() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::BotStarted { bot_id };
    match &event {
        GridEvent::BotStarted { bot_id: b } => assert_eq!(*b, bot_id),
        _ => panic!("Expected BotStarted"),
    }
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["BotStarted"]["bot_id"], serde_json::json!(bot_id));
}

#[test]
fn grid_event_grid_filled() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::GridFilled {
        bot_id, level: 3, side: "buy".to_string(), price: 51000.0, quantity: 0.001,
    };
    match &event {
        GridEvent::GridFilled { bot_id: b, level, side, price, quantity } => {
            assert_eq!(*b, bot_id);
            assert_eq!(*level, 3);
            assert_eq!(side, "buy");
            assert!((price - 51000.0).abs() < f64::EPSILON);
            assert!((quantity - 0.001).abs() < f64::EPSILON);
        }
        _ => panic!("Expected GridFilled"),
    }
    let json = serde_json::to_value(&event).unwrap();
    let filled = &json["GridFilled"];
    assert_eq!(filled["bot_id"], serde_json::json!(bot_id));
    assert_eq!(filled["level"], 3);
    assert_eq!(filled["side"], "buy");
    assert_eq!(filled["price"], 51000.0);
    assert_eq!(filled["quantity"], 0.001);
}
