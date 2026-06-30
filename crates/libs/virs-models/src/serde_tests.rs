//! Serde serialization/deserialization round-trip tests for all model types.

use chrono::Utc;
use uuid::Uuid;

use virs_types::enums::*;

use crate::{
    AutoBot, AutoTrade, CreateUserRequest, GridBot, GridTrade, LoginRequest, Order,
    StrategyStatus, User, UserRole, UserResponse,
};

// ============================================================
// TC-S1: Order serde round-trip
// ============================================================

#[test]
fn s1_1_order_roundtrip() {
    let now = Utc::now();
    let order = Order {
        id: "order_123".into(),
        client_order_id: Some("client_456".into()),
        symbol: "BTC/USDT".into(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        price: Some(50000.0),
        amount: 1.0,
        cost: Some(50000.0),
        filled: 0.5,
        remaining: 0.5,
        status: OrderStatus::PartiallyFilled,
        fee: 0.075,
        fee_currency: "BTC".into(),
        created_at: now,
        updated_at: now,
    };
    let json = serde_json::to_string(&order).unwrap();
    let de: Order = serde_json::from_str(&json).unwrap();
    assert_eq!(de, order);
}

// ============================================================
// TC-S2: User / UserResponse serde
// ============================================================

#[test]
fn s2_1_user_roundtrip() {
    let now = Utc::now();
    let user = User {
        id: Uuid::nil(),
        username: "admin".into(),
        password_hash: "hash".into(),
        role: UserRole::Admin,
        email: Some("admin@virs.com".into()),
        is_active: true,
        created_at: now,
        updated_at: now,
    };
    let json = serde_json::to_string(&user).unwrap();
    let de: User = serde_json::from_str(&json).unwrap();
    assert_eq!(de, user);
}

#[test]
fn s2_2_user_response_roundtrip() {
    let now = Utc::now();
    let response = UserResponse {
        id: Uuid::nil(),
        username: "admin".into(),
        role: UserRole::Admin,
        email: None,
        is_active: true,
        created_at: now,
    };
    let json = serde_json::to_string(&response).unwrap();
    let de: UserResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(de, response);
}

#[test]
fn s2_3_login_request_deserialize() {
    let json = r#"{"username":"admin","password":"secret"}"#;
    let req: LoginRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.username, "admin");
    assert_eq!(req.password, "secret");
}

#[test]
fn s2_4_create_user_request_with_role_none() {
    let json = r#"{"username":"newuser","password":"pass","email":null,"role":null}"#;
    let req: CreateUserRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.username, "newuser");
    assert_eq!(req.email, None);
    assert_eq!(req.role, None);
}

// ============================================================
// TC-S3: GridBot / GridTrade serde
// ============================================================

#[test]
fn s3_1_grid_bot_roundtrip() {
    let now = Utc::now();
    let bot = GridBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "grid_bot".into(),
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        market_type: "perpetual".into(),
        paper_mode: true,
        status: StrategyStatus::Running,
        upper_price: 50000.0,
        lower_price: 40000.0,
        grid_count: 10,
        grid_profit_pct: 0.5,
        quantity_per_grid: 0.01,
        leverage: 10,
        initial_capital: 10000.0,
        market_regime: None,
        ai_analysis: None,
        grid_levels_json: None,
        system_prompt: None,
        user_prompt: None,
        dynamic_adjust: false,
        adjust_interval_secs: 3600,
        last_adjusted_at: None,
        total_pnl: 500.0,
        unrealized_pnl: 100.0,
        total_trades: 20,
        grid_filled_count: 5,
        created_at: now,
        updated_at: now,
        started_at: Some(now),
        stopped_at: None,
    };
    let json = serde_json::to_string(&bot).unwrap();
    let de: GridBot = serde_json::from_str(&json).unwrap();
    assert_eq!(de.id, bot.id);
    assert_eq!(de.upper_price, bot.upper_price);
    assert_eq!(de.grid_count, bot.grid_count);
    assert_eq!(de.status, bot.status);
    assert_eq!(de.total_pnl, bot.total_pnl);
}

#[test]
fn s3_2_grid_trade_roundtrip() {
    let now = Utc::now();
    let trade = GridTrade {
        id: Uuid::nil(),
        bot_id: Uuid::nil(),
        user_id: Uuid::nil(),
        symbol: "BTC/USDT".into(),
        exchange: "binance".into(),
        grid_level: 3,
        open_side: "buy".into(),
        open_price: 45000.0,
        open_quantity: 0.1,
        open_order_id: Some("order_1".into()),
        opened_at: now,
        close_side: Some("sell".into()),
        close_price: Some(46000.0),
        close_quantity: Some(0.1),
        close_order_id: Some("order_2".into()),
        closed_at: Some(now),
        pnl: 10.0,
        pnl_pct: 2.2,
        status: "closed".into(),
        created_at: now,
    };
    let json = serde_json::to_string(&trade).unwrap();
    let de: GridTrade = serde_json::from_str(&json).unwrap();
    assert_eq!(de.open_price, trade.open_price);
    assert_eq!(de.pnl, trade.pnl);
    assert_eq!(de.status, trade.status);
}

// ============================================================
// TC-S4: AutoBot / AutoTrade serde
// ============================================================

#[test]
fn s4_1_auto_bot_roundtrip() {
    let now = Utc::now();
    let bot = AutoBot {
        id: Uuid::nil(),
        user_id: Uuid::nil(),
        name: "auto_bot".into(),
        symbol: "ETH/USDT".into(),
        exchange: "binance".into(),
        market_type: "perpetual".into(),
        paper_mode: false,
        status: "running".into(),
        leverage: 5,
        max_position_pct: 80.0,
        decide_interval_secs: 1800,
        initial_capital: 5000.0,
        position_id: None,
        market_regime: None,
        ai_analysis: None,
        system_prompt: None,
        user_prompt: None,
        total_pnl: 250.0,
        total_trades: 15,
        win_trades: 10,
        loss_trades: 5,
        last_decided_at: Some(now),
        created_at: now,
        updated_at: now,
        started_at: Some(now),
        stopped_at: None,
    };
    let json = serde_json::to_string(&bot).unwrap();
    let de: AutoBot = serde_json::from_str(&json).unwrap();
    assert_eq!(de.win_trades, bot.win_trades);
    assert_eq!(de.total_pnl, bot.total_pnl);
    assert_eq!(de.status, bot.status);
}

#[test]
fn s4_2_auto_trade_roundtrip() {
    let now = Utc::now();
    let trade = AutoTrade {
        id: Uuid::nil(),
        bot_id: Uuid::nil(),
        user_id: Uuid::nil(),
        symbol: "ETH/USDT".into(),
        exchange: "binance".into(),
        open_side: "buy".into(),
        open_price: 3000.0,
        open_quantity: 2.0,
        open_order_id: Some("open_order".into()),
        open_fee: 0.6,
        opened_at: now,
        close_side: Some("sell".into()),
        close_price: Some(3100.0),
        close_quantity: Some(2.0),
        close_order_id: Some("close_order".into()),
        close_fee: 0.62,
        closed_at: Some(now),
        pnl: 198.78,
        pnl_pct: 3.3,
        trigger_source: "ai".into(),
        close_reason: Some("take_profit".into()),
        status: "closed".into(),
        created_at: now,
    };
    let json = serde_json::to_string(&trade).unwrap();
    let de: AutoTrade = serde_json::from_str(&json).unwrap();
    assert_eq!(de.open_price, trade.open_price);
    assert_eq!(de.pnl, trade.pnl);
    assert_eq!(de.status, trade.status);
}
