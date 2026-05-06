use crate::bot::semi_automatic_grid::types::*;
use uuid::Uuid;

// ── GridEvent 序列化 roundtrip ──

#[test]
fn grid_event_bot_started_roundtrip() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::BotStarted { bot_id };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("BotStarted"));
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::BotStarted { bot_id: b } => assert_eq!(b, bot_id),
        _ => panic!("Expected BotStarted"),
    }
}

#[test]
fn grid_event_bot_stopped_roundtrip() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::BotStopped { bot_id, reason: "user requested".to_string() };
    let json = serde_json::to_string(&event).unwrap();
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::BotStopped { bot_id: b, reason } => {
            assert_eq!(b, bot_id);
            assert_eq!(reason, "user requested");
        }
        _ => panic!("Expected BotStopped"),
    }
}

#[test]
fn grid_event_bot_error_roundtrip() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::BotError { bot_id, error: "timeout".to_string() };
    let json = serde_json::to_string(&event).unwrap();
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::BotError { bot_id: b, error } => {
            assert_eq!(b, bot_id);
            assert_eq!(error, "timeout");
        }
        _ => panic!("Expected BotError"),
    }
}

#[test]
fn grid_event_grid_filled_roundtrip() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::GridFilled {
        bot_id, level: 3, side: "buy".to_string(), price: 51000.0, quantity: 0.001,
    };
    let json = serde_json::to_string(&event).unwrap();
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::GridFilled { bot_id: b, level, side, price, quantity } => {
            assert_eq!(b, bot_id);
            assert_eq!(level, 3);
            assert_eq!(side, "buy");
            assert!((price - 51000.0).abs() < f64::EPSILON);
            assert!((quantity - 0.001).abs() < f64::EPSILON);
        }
        _ => panic!("Expected GridFilled"),
    }
}

#[test]
fn grid_event_grid_trade_closed_roundtrip() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::GridTradeClosed { bot_id, level: 5, pnl: 12.5 };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("GridTradeClosed"));
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::GridTradeClosed { bot_id: b, level, pnl } => {
            assert_eq!(b, bot_id);
            assert_eq!(level, 5);
            assert!((pnl - 12.5).abs() < f64::EPSILON);
        }
        _ => panic!("Expected GridTradeClosed"),
    }
}

#[test]
fn grid_event_price_update_roundtrip() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::PriceUpdate { bot_id, price: 55000.0 };
    let json = serde_json::to_string(&event).unwrap();
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::PriceUpdate { bot_id: b, price } => {
            assert_eq!(b, bot_id);
            assert!((price - 55000.0).abs() < f64::EPSILON);
        }
        _ => panic!("Expected PriceUpdate"),
    }
}

#[test]
fn grid_event_status_update_roundtrip() {
    let bot_id = Uuid::new_v4();
    let state = GridState {
        bot_id,
        symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(),
        levels: vec![GridLevel {
            level: 0, price: 50000.0, buy_price: 50000.0, sell_price: 50250.0,
            quantity: 0.002, buy_order_id: Some(Uuid::new_v4()), sell_order_id: None,
            buy_filled: true, sell_filled: false, hold_quantity: 0.002,
        }],
        current_price: 55000.0,
        total_pnl: 100.0,
        total_trades: 5,
        grid_filled_count: 3,
        last_tick_at: chrono::Utc::now(),
    };
    let event = GridEvent::StatusUpdate { bot_id, state };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("StatusUpdate"));
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::StatusUpdate { bot_id: b, state } => {
            assert_eq!(b, bot_id);
            assert_eq!(state.levels.len(), 1);
            assert!(state.levels[0].buy_filled);
            assert!(state.levels[0].buy_order_id.is_some());
            assert!((state.levels[0].hold_quantity - 0.002).abs() < f64::EPSILON);
        }
        _ => panic!("Expected StatusUpdate"),
    }
}

// ── GridLevel 序列化 roundtrip ──

#[test]
fn grid_level_roundtrip_default() {
    let level = GridLevel {
        level: 3, price: 52000.0, buy_price: 52000.0, sell_price: 52260.0,
        quantity: 0.0019, buy_order_id: None, sell_order_id: None,
        buy_filled: false, sell_filled: false, hold_quantity: 0.0,
    };
    let json = serde_json::to_string(&level).unwrap();
    let de: GridLevel = serde_json::from_str(&json).unwrap();
    assert_eq!(de.level, 3);
    assert!((de.price - 52000.0).abs() < f64::EPSILON);
    assert!((de.buy_price - 52000.0).abs() < f64::EPSILON);
    assert!((de.sell_price - 52260.0).abs() < f64::EPSILON);
    assert!((de.quantity - 0.0019).abs() < f64::EPSILON);
    assert!(de.buy_order_id.is_none());
    assert!(de.sell_order_id.is_none());
    assert!(!de.buy_filled);
    assert!(!de.sell_filled);
    assert!((de.hold_quantity).abs() < f64::EPSILON);
}

#[test]
fn grid_level_roundtrip_with_order_ids() {
    let buy_id = Uuid::new_v4();
    let sell_id = Uuid::new_v4();
    let level = GridLevel {
        level: 3, price: 52000.0, buy_price: 52000.0, sell_price: 52260.0,
        quantity: 0.0019, buy_order_id: Some(buy_id), sell_order_id: Some(sell_id),
        buy_filled: true, sell_filled: false, hold_quantity: 0.0019,
    };
    let json = serde_json::to_string(&level).unwrap();
    let de: GridLevel = serde_json::from_str(&json).unwrap();
    assert_eq!(de.buy_order_id, Some(buy_id));
    assert_eq!(de.sell_order_id, Some(sell_id));
    assert!(de.buy_filled);
    assert!(!de.sell_filled);
    assert!((de.hold_quantity - 0.0019).abs() < f64::EPSILON);
}

#[test]
fn grid_level_roundtrip_sell_filled_hold_zero() {
    let level = GridLevel {
        level: 1, price: 51000.0, buy_price: 51000.0, sell_price: 51255.0,
        quantity: 0.0019, buy_order_id: None, sell_order_id: None,
        buy_filled: true, sell_filled: true, hold_quantity: 0.0,
    };
    let json = serde_json::to_string(&level).unwrap();
    let de: GridLevel = serde_json::from_str(&json).unwrap();
    assert!(de.buy_filled);
    assert!(de.sell_filled);
    assert!((de.hold_quantity).abs() < f64::EPSILON);
}

#[test]
fn grid_level_roundtrip_negative_hold_quantity() {
    let level = GridLevel {
        level: 2, price: 52000.0, buy_price: 52000.0, sell_price: 52260.0,
        quantity: 0.0019, buy_order_id: None, sell_order_id: None,
        buy_filled: true, sell_filled: true, hold_quantity: -0.001,
    };
    let json = serde_json::to_string(&level).unwrap();
    let de: GridLevel = serde_json::from_str(&json).unwrap();
    assert!(de.hold_quantity < 0.0);
}

#[test]
fn grid_level_roundtrip_zero_prices() {
    let level = GridLevel {
        level: 0, price: 0.0, buy_price: 0.0, sell_price: 0.0,
        quantity: 0.0, buy_order_id: None, sell_order_id: None,
        buy_filled: false, sell_filled: false, hold_quantity: 0.0,
    };
    let json = serde_json::to_string(&level).unwrap();
    let de: GridLevel = serde_json::from_str(&json).unwrap();
    assert!((de.price).abs() < f64::EPSILON);
    assert!((de.buy_price).abs() < f64::EPSILON);
    assert!((de.sell_price).abs() < f64::EPSILON);
    assert!((de.quantity).abs() < f64::EPSILON);
}

// ── GridState 序列化 roundtrip ──

#[test]
fn grid_state_roundtrip_empty_levels() {
    let bot_id = Uuid::new_v4();
    let state = GridState {
        bot_id, symbol: "BTCUSDT".to_string(), exchange: "binance".to_string(),
        levels: vec![], current_price: 55000.0, total_pnl: 0.0,
        total_trades: 0, grid_filled_count: 0, last_tick_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&state).unwrap();
    let de: GridState = serde_json::from_str(&json).unwrap();
    assert_eq!(de.bot_id, bot_id);
    assert!(de.levels.is_empty());
    assert!((de.total_pnl).abs() < f64::EPSILON);
    assert_eq!(de.total_trades, 0);
}

#[test]
fn grid_state_roundtrip_multiple_levels() {
    let bot_id = Uuid::new_v4();
    let levels = vec![
        GridLevel {
            level: 0, price: 50000.0, buy_price: 50000.0, sell_price: 50250.0,
            quantity: 0.002, buy_order_id: None, sell_order_id: None,
            buy_filled: false, sell_filled: false, hold_quantity: 0.0,
        },
        GridLevel {
            level: 1, price: 51000.0, buy_price: 51000.0, sell_price: 51255.0,
            quantity: 0.0019, buy_order_id: Some(Uuid::new_v4()), sell_order_id: None,
            buy_filled: true, sell_filled: false, hold_quantity: 0.0019,
        },
        GridLevel {
            level: 2, price: 52000.0, buy_price: 52000.0, sell_price: 52260.0,
            quantity: 0.0019, buy_order_id: None, sell_order_id: Some(Uuid::new_v4()),
            buy_filled: true, sell_filled: true, hold_quantity: 0.0,
        },
    ];
    let state = GridState {
        bot_id, symbol: "BTCUSDT".to_string(), exchange: "binance".to_string(),
        levels, current_price: 55000.0, total_pnl: 50.0,
        total_trades: 2, grid_filled_count: 1, last_tick_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&state).unwrap();
    let de: GridState = serde_json::from_str(&json).unwrap();
    assert_eq!(de.levels.len(), 3);
    assert!(!de.levels[0].buy_filled);
    assert!(de.levels[1].buy_filled);
    assert!(de.levels[1].buy_order_id.is_some());
    assert!(de.levels[2].buy_filled);
    assert!(de.levels[2].sell_filled);
    assert!((de.total_pnl - 50.0).abs() < f64::EPSILON);
}

#[test]
fn grid_state_roundtrip_negative_pnl() {
    let bot_id = Uuid::new_v4();
    let state = GridState {
        bot_id, symbol: "BTCUSDT".to_string(), exchange: "binance".to_string(),
        levels: vec![], current_price: 45000.0, total_pnl: -100.0,
        total_trades: 5, grid_filled_count: 3, last_tick_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&state).unwrap();
    let de: GridState = serde_json::from_str(&json).unwrap();
    assert!(de.total_pnl < 0.0);
    assert!((de.total_pnl - (-100.0)).abs() < f64::EPSILON);
}

// ── GridEvent 边界场景 ──

#[test]
fn grid_event_grid_trade_closed_negative_pnl() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::GridTradeClosed { bot_id, level: 2, pnl: -5.0 };
    let json = serde_json::to_string(&event).unwrap();
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::GridTradeClosed { pnl, .. } => assert!(pnl < 0.0),
        _ => panic!("Expected GridTradeClosed"),
    }
}

#[test]
fn grid_event_grid_trade_closed_zero_pnl() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::GridTradeClosed { bot_id, level: 0, pnl: 0.0 };
    let json = serde_json::to_string(&event).unwrap();
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::GridTradeClosed { pnl, level, .. } => {
            assert!((pnl).abs() < f64::EPSILON);
            assert_eq!(level, 0);
        }
        _ => panic!("Expected GridTradeClosed"),
    }
}

#[test]
fn grid_event_grid_filled_sell_side() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::GridFilled {
        bot_id, level: 7, side: "sell".to_string(), price: 58000.0, quantity: 0.005,
    };
    let json = serde_json::to_string(&event).unwrap();
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::GridFilled { side, level, price, quantity, .. } => {
            assert_eq!(side, "sell");
            assert_eq!(level, 7);
            assert!((price - 58000.0).abs() < f64::EPSILON);
            assert!((quantity - 0.005).abs() < f64::EPSILON);
        }
        _ => panic!("Expected GridFilled"),
    }
}

#[test]
fn grid_event_bot_stopped_empty_reason() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::BotStopped { bot_id, reason: String::new() };
    let json = serde_json::to_string(&event).unwrap();
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::BotStopped { reason, .. } => assert!(reason.is_empty()),
        _ => panic!("Expected BotStopped"),
    }
}

#[test]
fn grid_event_bot_error_long_error_message() {
    let bot_id = Uuid::new_v4();
    let long_msg = "x".repeat(10000);
    let event = GridEvent::BotError { bot_id, error: long_msg.clone() };
    let json = serde_json::to_string(&event).unwrap();
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::BotError { error, .. } => assert_eq!(error, long_msg),
        _ => panic!("Expected BotError"),
    }
}

#[test]
fn grid_event_price_update_zero_price() {
    let bot_id = Uuid::new_v4();
    let event = GridEvent::PriceUpdate { bot_id, price: 0.0 };
    let json = serde_json::to_string(&event).unwrap();
    let de: GridEvent = serde_json::from_str(&json).unwrap();
    match de {
        GridEvent::PriceUpdate { price, .. } => assert!((price).abs() < f64::EPSILON),
        _ => panic!("Expected PriceUpdate"),
    }
}

// ── GridCommand Debug ──

#[test]
fn grid_command_debug_all_variants() {
    let bot_id = Uuid::new_v4();
    assert!(format!("{:?}", GridCommand::StartBot { bot_id }).contains("StartBot"));
    assert!(format!("{:?}", GridCommand::StopBot { bot_id }).contains("StopBot"));
    assert!(format!("{:?}", GridCommand::PauseBot { bot_id }).contains("PauseBot"));
    assert!(format!("{:?}", GridCommand::ResumeBot { bot_id }).contains("ResumeBot"));
    assert!(format!("{:?}", GridCommand::DeleteBot { bot_id }).contains("DeleteBot"));
    assert!(format!("{:?}", GridCommand::AdjustGrid { bot_id }).contains("AdjustGrid"));
    assert!(format!("{:?}", GridCommand::Shutdown).contains("Shutdown"));
}

// ── GridLevel 边界值 ──

#[test]
fn grid_level_max_level_number() {
    let level = GridLevel {
        level: i32::MAX, price: 1.0, buy_price: 1.0, sell_price: 1.01,
        quantity: 100.0, buy_order_id: None, sell_order_id: None,
        buy_filled: false, sell_filled: false, hold_quantity: 0.0,
    };
    assert_eq!(level.level, i32::MAX);
}

#[test]
fn grid_level_very_small_quantity() {
    let level = GridLevel {
        level: 0, price: 100000.0, buy_price: 100000.0, sell_price: 100500.0,
        quantity: 0.00000001, buy_order_id: None, sell_order_id: None,
        buy_filled: false, sell_filled: false, hold_quantity: 0.00000001,
    };
    assert!((level.quantity - 0.00000001).abs() < 1e-12);
    assert!((level.hold_quantity - 0.00000001).abs() < 1e-12);
}
