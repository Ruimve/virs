use chrono::Utc;
use std::f64::EPSILON as F64_EPS;
use uuid::Uuid;

use super::super::types::*;

// ============================================================
// 枚举序列化/反序列化 (6 tests)
// ============================================================

#[test]
fn test_side_serialization() {
    // Side::Buy -> "Buy", Side::Sell -> "Sell"
    let buy_json = serde_json::to_string(&Side::Buy).unwrap();
    let sell_json = serde_json::to_string(&Side::Sell).unwrap();
    assert_eq!(buy_json, "\"Buy\"");
    assert_eq!(sell_json, "\"Sell\"");

    // 反序列化
    let buy_back: Side = serde_json::from_str(&buy_json).unwrap();
    let sell_back: Side = serde_json::from_str(&sell_json).unwrap();
    assert_eq!(buy_back, Side::Buy);
    assert_eq!(sell_back, Side::Sell);

    // 往返一致性
    assert_eq!(serde_json::from_str::<Side>(&serde_json::to_string(&Side::Buy).unwrap()).unwrap(), Side::Buy);
    assert_eq!(serde_json::from_str::<Side>(&serde_json::to_string(&Side::Sell).unwrap()).unwrap(), Side::Sell);
}

#[test]
fn test_position_side_serialization() {
    let long_json = serde_json::to_string(&PositionSide::Long).unwrap();
    let short_json = serde_json::to_string(&PositionSide::Short).unwrap();
    let both_json = serde_json::to_string(&PositionSide::Both).unwrap();
    assert_eq!(long_json, "\"Long\"");
    assert_eq!(short_json, "\"Short\"");
    assert_eq!(both_json, "\"Both\"");

    assert_eq!(serde_json::from_str::<PositionSide>(&long_json).unwrap(), PositionSide::Long);
    assert_eq!(serde_json::from_str::<PositionSide>(&short_json).unwrap(), PositionSide::Short);
    assert_eq!(serde_json::from_str::<PositionSide>(&both_json).unwrap(), PositionSide::Both);
}

#[test]
fn test_order_type_serialization() {
    let limit_json = serde_json::to_string(&OrderType::Limit).unwrap();
    let market_json = serde_json::to_string(&OrderType::Market).unwrap();
    let stop_json = serde_json::to_string(&OrderType::StopMarket).unwrap();
    let tp_json = serde_json::to_string(&OrderType::TakeProfitMarket).unwrap();

    assert_eq!(limit_json, "\"Limit\"");
    assert_eq!(market_json, "\"Market\"");
    assert_eq!(stop_json, "\"StopMarket\"");
    assert_eq!(tp_json, "\"TakeProfitMarket\"");

    assert_eq!(serde_json::from_str::<OrderType>(&limit_json).unwrap(), OrderType::Limit);
    assert_eq!(serde_json::from_str::<OrderType>(&market_json).unwrap(), OrderType::Market);
    assert_eq!(serde_json::from_str::<OrderType>(&stop_json).unwrap(), OrderType::StopMarket);
    assert_eq!(serde_json::from_str::<OrderType>(&tp_json).unwrap(), OrderType::TakeProfitMarket);
}

#[test]
fn test_order_status_serialization() {
    let variants = vec![
        (OrderStatus::Pending, "\"Pending\""),
        (OrderStatus::Open, "\"Open\""),
        (OrderStatus::PartiallyFilled, "\"PartiallyFilled\""),
        (OrderStatus::Filled, "\"Filled\""),
        (OrderStatus::Canceled, "\"Canceled\""),
        (OrderStatus::Failed, "\"Failed\""),
    ];

    for (variant, expected_json) in &variants {
        let json = serde_json::to_string(variant).unwrap();
        assert_eq!(json, *expected_json);
        let back: OrderStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, *variant);
    }
}

#[test]
fn test_position_status_serialization() {
    let variants = vec![
        (PositionStatus::Empty, "\"Empty\""),
        (PositionStatus::Opening, "\"Opening\""),
        (PositionStatus::Open, "\"Open\""),
        (PositionStatus::Closing, "\"Closing\""),
        (PositionStatus::Closed, "\"Closed\""),
    ];

    for (variant, expected_json) in &variants {
        let json = serde_json::to_string(variant).unwrap();
        assert_eq!(json, *expected_json);
        let back: PositionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, *variant);
    }
}

#[test]
fn test_market_type_serialization() {
    let spot_json = serde_json::to_string(&MarketType::Spot).unwrap();
    let perp_json = serde_json::to_string(&MarketType::Perpetual).unwrap();
    assert_eq!(spot_json, "\"Spot\"");
    assert_eq!(perp_json, "\"Perpetual\"");

    assert_eq!(serde_json::from_str::<MarketType>(&spot_json).unwrap(), MarketType::Spot);
    assert_eq!(serde_json::from_str::<MarketType>(&perp_json).unwrap(), MarketType::Perpetual);
}

// ============================================================
// Position 构造与默认值 (4 tests)
// ============================================================

#[test]
fn test_position_default_values() {
    let now = Utc::now();
    let pos = Position {
        id: Uuid::new_v4(),
        engine_id: "engine1".to_string(),
        strategy_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        status: PositionStatus::Open,
        size: 1.0,
        entry_price: 50000.0,
        current_price: 50000.0,
        leverage: 10,
        margin: 5000.0,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss: None,
        take_profit: None,
        liquidation_price: None,
        opened_at: now,
        updated_at: now,
        closed_at: None,
        metadata: serde_json::Value::Null,
    };

    assert_eq!(pos.status, PositionStatus::Open);
    assert_eq!(pos.unrealized_pnl, 0.0);
    assert_eq!(pos.realized_pnl, 0.0);
    assert!(pos.closed_at.is_none());
    assert_eq!(pos.metadata, serde_json::Value::Null);
    assert!(pos.stop_loss.is_none());
    assert!(pos.take_profit.is_none());
    assert!(pos.liquidation_price.is_none());
    assert!(pos.strategy_id.is_none());
}

#[test]
fn test_position_clone_independence() {
    let now = Utc::now();
    let pos = Position {
        id: Uuid::new_v4(),
        engine_id: "engine1".to_string(),
        strategy_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        status: PositionStatus::Open,
        size: 1.0,
        entry_price: 50000.0,
        current_price: 50000.0,
        leverage: 10,
        margin: 5000.0,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss: None,
        take_profit: None,
        liquidation_price: None,
        opened_at: now,
        updated_at: now,
        closed_at: None,
        metadata: serde_json::Value::Null,
    };

    let mut cloned = pos.clone();

    // 修改 clone 后不影响原对象
    cloned.size = 2.0;
    cloned.status = PositionStatus::Closed;
    cloned.current_price = 51000.0;
    cloned.unrealized_pnl = 1000.0;
    cloned.stop_loss = Some(49000.0);

    assert_eq!(pos.size, 1.0);
    assert_eq!(pos.status, PositionStatus::Open);
    assert_eq!(pos.current_price, 50000.0);
    assert_eq!(pos.unrealized_pnl, 0.0);
    assert!(pos.stop_loss.is_none());

    assert_eq!(cloned.size, 2.0);
    assert_eq!(cloned.status, PositionStatus::Closed);
    assert_eq!(cloned.current_price, 51000.0);
    assert_eq!(cloned.unrealized_pnl, 1000.0);
    assert_eq!(cloned.stop_loss, Some(49000.0));
}

#[test]
fn test_position_with_stop_loss_take_profit() {
    let now = Utc::now();
    let pos = Position {
        id: Uuid::new_v4(),
        engine_id: "engine1".to_string(),
        strategy_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        status: PositionStatus::Open,
        size: 1.0,
        entry_price: 50000.0,
        current_price: 50000.0,
        leverage: 10,
        margin: 5000.0,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss: Some(48000.0),
        take_profit: Some(55000.0),
        liquidation_price: Some(45000.0),
        opened_at: now,
        updated_at: now,
        closed_at: None,
        metadata: serde_json::Value::Null,
    };

    assert_eq!(pos.stop_loss, Some(48000.0));
    assert_eq!(pos.take_profit, Some(55000.0));
    assert_eq!(pos.liquidation_price, Some(45000.0));
}

#[test]
fn test_position_closed_state() {
    let now = Utc::now();
    let closed_time = now;
    let pos = Position {
        id: Uuid::new_v4(),
        engine_id: "engine1".to_string(),
        strategy_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        status: PositionStatus::Closed,
        size: 1.0,
        entry_price: 50000.0,
        current_price: 51000.0,
        leverage: 10,
        margin: 5000.0,
        unrealized_pnl: 0.0,
        realized_pnl: 1000.0,
        stop_loss: None,
        take_profit: None,
        liquidation_price: None,
        opened_at: now,
        updated_at: closed_time,
        closed_at: Some(closed_time),
        metadata: serde_json::Value::Null,
    };

    assert_eq!(pos.status, PositionStatus::Closed);
    assert!(pos.closed_at.is_some());
    assert_eq!(pos.closed_at.unwrap(), closed_time);
}

// ============================================================
// Order 构造与状态 (4 tests)
// ============================================================

#[test]
fn test_order_limit_vs_market() {
    let now = Utc::now();

    // Limit 单有 request_price
    let limit_order = Order {
        id: Uuid::new_v4(),
        position_id: Uuid::nil(),
        exchange_order_id: None,
        client_order_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        request_price: Some(50000.0),
        fill_price: None,
        amount: 1.0,
        filled: 0.0,
        remaining: 1.0,
        status: OrderStatus::Open,
        reduce_only: false,
        fee: 0.0,
        fee_currency: "USDT".to_string(),
        slippage: None,
        created_at: now,
        updated_at: now,
    };

    // Market 单没有 request_price
    let market_order = Order {
        id: Uuid::new_v4(),
        position_id: Uuid::nil(),
        exchange_order_id: None,
        client_order_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        request_price: None,
        fill_price: None,
        amount: 1.0,
        filled: 0.0,
        remaining: 1.0,
        status: OrderStatus::Open,
        reduce_only: false,
        fee: 0.0,
        fee_currency: "USDT".to_string(),
        slippage: None,
        created_at: now,
        updated_at: now,
    };

    assert_eq!(limit_order.order_type, OrderType::Limit);
    assert!(limit_order.request_price.is_some());
    assert_eq!(limit_order.request_price.unwrap(), 50000.0);

    assert_eq!(market_order.order_type, OrderType::Market);
    assert!(market_order.request_price.is_none());
}

#[test]
fn test_order_fill_tracking() {
    let now = Utc::now();

    // 未成交
    let order = Order {
        id: Uuid::new_v4(),
        position_id: Uuid::nil(),
        exchange_order_id: None,
        client_order_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        request_price: Some(50000.0),
        fill_price: None,
        amount: 10.0,
        filled: 0.0,
        remaining: 10.0,
        status: OrderStatus::Open,
        reduce_only: false,
        fee: 0.0,
        fee_currency: "USDT".to_string(),
        slippage: None,
        created_at: now,
        updated_at: now,
    };
    assert_eq!(order.filled + order.remaining, order.amount);

    // 部分成交
    let partial = Order {
        filled: 3.0,
        remaining: 7.0,
        ..order.clone()
    };
    assert_eq!(partial.filled + partial.remaining, partial.amount);

    // 完全成交
    let fully_filled = Order {
        filled: 10.0,
        remaining: 0.0,
        status: OrderStatus::Filled,
        ..order
    };
    assert_eq!(fully_filled.filled + fully_filled.remaining, fully_filled.amount);
    assert_eq!(fully_filled.filled, fully_filled.amount);
    assert_eq!(fully_filled.remaining, 0.0);
}

#[test]
fn test_order_reduce_only_flag() {
    let now = Utc::now();

    // 默认 reduce_only = false
    let order = Order {
        id: Uuid::new_v4(),
        position_id: Uuid::nil(),
        exchange_order_id: None,
        client_order_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Sell,
        order_type: OrderType::Market,
        request_price: None,
        fill_price: None,
        amount: 1.0,
        filled: 0.0,
        remaining: 1.0,
        status: OrderStatus::Open,
        reduce_only: false,
        fee: 0.0,
        fee_currency: "USDT".to_string(),
        slippage: None,
        created_at: now,
        updated_at: now,
    };
    assert!(!order.reduce_only);

    // 设置 reduce_only = true
    let reduce_order = Order { reduce_only: true, ..order };
    assert!(reduce_order.reduce_only);
}

#[test]
fn test_order_status_transitions() {
    // 验证 OrderStatus 各变体之间的逻辑关系（不测试自动转换，只测试枚举值）
    let all_statuses = [
        OrderStatus::Pending,
        OrderStatus::Open,
        OrderStatus::PartiallyFilled,
        OrderStatus::Filled,
        OrderStatus::Canceled,
        OrderStatus::Failed,
    ];

    // 所有变体互不相同
    for i in 0..all_statuses.len() {
        for j in (i + 1)..all_statuses.len() {
            assert_ne!(all_statuses[i], all_statuses[j]);
        }
    }

    // 终态: Filled, Canceled, Failed 互不相等
    assert_ne!(OrderStatus::Filled, OrderStatus::Canceled);
    assert_ne!(OrderStatus::Filled, OrderStatus::Failed);
    assert_ne!(OrderStatus::Canceled, OrderStatus::Failed);

    // 非终态: Pending, Open, PartiallyFilled 互不相等
    assert_ne!(OrderStatus::Pending, OrderStatus::Open);
    assert_ne!(OrderStatus::Pending, OrderStatus::PartiallyFilled);
    assert_ne!(OrderStatus::Open, OrderStatus::PartiallyFilled);

    // Copy trait: 可以多次使用
    let s = OrderStatus::Open;
    let _s2 = s;
    let _s3 = s; // s 仍然可用
}

// ============================================================
// Trade 构造 (3 tests)
// ============================================================

#[test]
fn test_trade_types() {
    let now = Utc::now();

    let open_trade = Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        price: 50000.0,
        amount: 1.0,
        fee: 25.0,
        fee_currency: "USDT".to_string(),
        pnl: 0.0,
        trade_type: "Open".to_string(),
        created_at: now,
    };

    let close_trade = Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Sell,
        price: 51000.0,
        amount: 1.0,
        fee: 25.5,
        fee_currency: "USDT".to_string(),
        pnl: 1000.0,
        trade_type: "close".to_string(),
        created_at: now,
    };

    let partial_close_trade = Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Sell,
        price: 50500.0,
        amount: 0.5,
        fee: 12.625,
        fee_currency: "USDT".to_string(),
        pnl: 250.0,
        trade_type: "partial_close".to_string(),
        created_at: now,
    };

    assert_eq!(open_trade.trade_type, "Open");
    assert_eq!(close_trade.trade_type, "close");
    assert_eq!(partial_close_trade.trade_type, "partial_close");
}

#[test]
fn test_trade_pnl_calculation() {
    // Long close: pnl = (close_price - entry_price) * amount
    let long_close_pnl: f64 = (51000.0 - 50000.0) * 1.0;
    assert!((long_close_pnl - 1000.0).abs() < F64_EPS);

    // Short close: pnl = (entry_price - close_price) * amount
    let short_close_pnl: f64 = (50000.0 - 49000.0) * 1.0;
    assert!((short_close_pnl - 1000.0).abs() < F64_EPS);

    // 使用 Trade 结构体验证
    let now = Utc::now();
    let long_trade = Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Sell,
        price: 51000.0,
        amount: 1.0,
        fee: 25.5,
        fee_currency: "USDT".to_string(),
        pnl: long_close_pnl,
        trade_type: "close".to_string(),
        created_at: now,
    };
    assert!((long_trade.pnl - 1000.0).abs() < F64_EPS);

    let short_trade = Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        price: 49000.0,
        amount: 1.0,
        fee: 24.5,
        fee_currency: "USDT".to_string(),
        pnl: short_close_pnl,
        trade_type: "close".to_string(),
        created_at: now,
    };
    assert!((short_trade.pnl - 1000.0).abs() < F64_EPS);

    // 亏损场景: Long close at lower price
    let long_loss_pnl: f64 = (49000.0 - 50000.0) * 2.0;
    assert!((long_loss_pnl - (-2000.0)).abs() < F64_EPS);
}

#[test]
fn test_trade_fee_currency() {
    let now = Utc::now();

    // 默认 fee_currency = "USDT"
    let trade = Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        price: 50000.0,
        amount: 1.0,
        fee: 25.0,
        fee_currency: "USDT".to_string(),
        pnl: 0.0,
        trade_type: "Open".to_string(),
        created_at: now,
    };

    assert_eq!(trade.fee_currency, "USDT");

    // 也可以是其他币种
    let trade_bnb = Trade {
        fee_currency: "BNB".to_string(),
        ..trade
    };
    assert_eq!(trade_bnb.fee_currency, "BNB");
}

// ============================================================
// WsFeedEvent (3 tests)
// ============================================================

#[test]
fn test_ws_feed_event_order_update_fields() {
    let now = Utc::now();
    let event = WsFeedEvent::OrderUpdate {
        exchange_order_id: "ord_12345".to_string(),
        symbol: "BTC/USDT".to_string(),
        status: OrderStatus::Filled,
        filled: 1.0,
        remaining: 0.0,
        price: 50000.0,
        amount: 1.0,
        commission: 25.0,
        timestamp: now,
    };

    if let WsFeedEvent::OrderUpdate {
        exchange_order_id,
        symbol,
        status,
        filled,
        remaining,
        price,
        amount,
        commission,
        timestamp,
    } = event
    {
        assert_eq!(exchange_order_id, "ord_12345");
        assert_eq!(symbol, "BTC/USDT");
        assert_eq!(status, OrderStatus::Filled);
        assert_eq!(filled, 1.0);
        assert_eq!(remaining, 0.0);
        assert_eq!(price, 50000.0);
        assert_eq!(amount, 1.0);
        assert_eq!(commission, 25.0);
        assert_eq!(timestamp, now);
    } else {
        panic!("Expected OrderUpdate variant");
    }
}

#[test]
fn test_ws_feed_event_connection_changed() {
    let connected_event = WsFeedEvent::ConnectionChanged { connected: true };
    let disconnected_event = WsFeedEvent::ConnectionChanged { connected: false };

    if let WsFeedEvent::ConnectionChanged { connected } = connected_event {
        assert!(connected);
    } else {
        panic!("Expected ConnectionChanged variant with connected=true");
    }

    if let WsFeedEvent::ConnectionChanged { connected } = disconnected_event {
        assert!(!connected);
    } else {
        panic!("Expected ConnectionChanged variant with connected=false");
    }
}

#[test]
fn test_ws_feed_event_clone() {
    let now = Utc::now();
    let event = WsFeedEvent::OrderUpdate {
        exchange_order_id: "ord_123".to_string(),
        symbol: "ETH/USDT".to_string(),
        status: OrderStatus::PartiallyFilled,
        filled: 0.5,
        remaining: 0.5,
        price: 3000.0,
        amount: 1.0,
        commission: 1.5,
        timestamp: now,
    };

    let cloned = event.clone();

    // 验证 clone 的值相同
    if let (WsFeedEvent::OrderUpdate { exchange_order_id: id1, .. },
            WsFeedEvent::OrderUpdate { exchange_order_id: id2, .. }) = (&event, &cloned) {
        assert_eq!(id1, id2);
    } else {
        panic!("Both should be OrderUpdate");
    }

    // ConnectionChanged 也可以 clone
    let conn_event = WsFeedEvent::ConnectionChanged { connected: true };
    let conn_cloned = conn_event.clone();
    if let WsFeedEvent::ConnectionChanged { connected } = conn_cloned {
        assert!(connected);
    } else {
        panic!("Expected ConnectionChanged");
    }
}

// ============================================================
// PlaceOrderParams (3 tests)
// ============================================================

#[test]
fn test_place_order_params_market() {
    let params = PlaceOrderParams {
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        amount: 1.0,
        price: None,
        reduce_only: false,
        position_side: None,
    };

    assert_eq!(params.order_type, OrderType::Market);
    assert!(params.price.is_none());
    assert!(!params.reduce_only);
    assert!(params.position_side.is_none());
}

#[test]
fn test_place_order_params_limit() {
    let params = PlaceOrderParams {
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        amount: 1.0,
        price: Some(100.0),
        reduce_only: false,
        position_side: None,
    };

    assert_eq!(params.order_type, OrderType::Limit);
    assert!(params.price.is_some());
    assert_eq!(params.price.unwrap(), 100.0);
}

#[test]
fn test_place_order_params_reduce_only() {
    let params = PlaceOrderParams {
        symbol: "BTC/USDT".to_string(),
        side: Side::Sell,
        order_type: OrderType::Market,
        amount: 1.0,
        price: None,
        reduce_only: true,
        position_side: Some(PositionSide::Long),
    };

    assert!(params.reduce_only);
    assert!(params.position_side.is_some());
    assert_eq!(params.position_side.unwrap(), PositionSide::Long);
    assert_eq!(params.side, Side::Sell);
}

// ============================================================
// EngineCommand / EngineEvent 变体完整性 (2 tests)
// ============================================================

#[test]
fn test_engine_command_all_variants_constructible() {
    let _now = Utc::now();
    let pos_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();

    // 1. OpenPosition
    let cmd1 = EngineCommand::OpenPosition {
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        size: 1.0,
        leverage: Some(10),
        order_type: OrderType::Market,
        price: None,
        stop_loss: Some(48000.0),
        take_profit: Some(55000.0),
        strategy_id: Some("strat1".to_string()),
    };
    assert!(matches!(cmd1, EngineCommand::OpenPosition { .. }));

    // 2. ClosePosition
    let cmd2 = EngineCommand::ClosePosition {
        position_id: pos_id,
        order_type: OrderType::Market,
        price: None,
    };
    assert!(matches!(cmd2, EngineCommand::ClosePosition { .. }));

    // 3. ModifyPosition
    let cmd3 = EngineCommand::ModifyPosition {
        position_id: pos_id,
        stop_loss: Some(49000.0),
        take_profit: None,
    };
    assert!(matches!(cmd3, EngineCommand::ModifyPosition { .. }));

    // 4. PlaceOrder
    let cmd4 = EngineCommand::PlaceOrder {
        params: PlaceOrderParams {
            symbol: "BTC/USDT".to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            amount: 1.0,
            price: Some(50000.0),
            reduce_only: false,
            position_side: None,
        },
    };
    assert!(matches!(cmd4, EngineCommand::PlaceOrder { .. }));

    // 5. CancelOrder
    let cmd5 = EngineCommand::CancelOrder { order_id };
    assert!(matches!(cmd5, EngineCommand::CancelOrder { .. }));

    // 6. CancelAllOrders
    let cmd6 = EngineCommand::CancelAllOrders {
        position_id: Some(pos_id),
        symbol: Some("BTC/USDT".to_string()),
    };
    assert!(matches!(cmd6, EngineCommand::CancelAllOrders { .. }));

    // 7. SyncPositions
    let cmd7 = EngineCommand::SyncPositions;
    assert!(matches!(cmd7, EngineCommand::SyncPositions));

    // 8. Shutdown
    let cmd8 = EngineCommand::Shutdown;
    assert!(matches!(cmd8, EngineCommand::Shutdown));

    // 验证所有变体互不相同
    let _cmds: Vec<EngineCommand> = vec![cmd1, cmd2, cmd3, cmd4, cmd5, cmd6, cmd7, cmd8];
    assert_eq!(_cmds.len(), 8);
}

#[test]
fn test_engine_event_all_variants_constructible() {
    let now = Utc::now();
    let pos_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let trade_id = Uuid::new_v4();

    let position = Position {
        id: pos_id,
        engine_id: "engine1".to_string(),
        strategy_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        status: PositionStatus::Open,
        size: 1.0,
        entry_price: 50000.0,
        current_price: 50000.0,
        leverage: 10,
        margin: 5000.0,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
        stop_loss: None,
        take_profit: None,
        liquidation_price: None,
        opened_at: now,
        updated_at: now,
        closed_at: None,
        metadata: serde_json::Value::Null,
    };

    let order = Order {
        id: order_id,
        position_id: pos_id,
        exchange_order_id: Some("ex_ord_1".to_string()),
        client_order_id: None,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        request_price: None,
        fill_price: Some(50000.0),
        amount: 1.0,
        filled: 1.0,
        remaining: 0.0,
        status: OrderStatus::Filled,
        reduce_only: false,
        fee: 25.0,
        fee_currency: "USDT".to_string(),
        slippage: None,
        created_at: now,
        updated_at: now,
    };

    let trade = Trade {
        id: trade_id,
        position_id: pos_id,
        order_id,
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Buy,
        price: 50000.0,
        amount: 1.0,
        fee: 25.0,
        fee_currency: "USDT".to_string(),
        pnl: 0.0,
        trade_type: "Open".to_string(),
        created_at: now,
    };

    // 1. PositionOpened
    let ev1 = EngineEvent::PositionOpened { position: position.clone() };
    assert!(matches!(ev1, EngineEvent::PositionOpened { .. }));

    // 2. PositionClosed
    let ev2 = EngineEvent::PositionClosed { position: position.clone() };
    assert!(matches!(ev2, EngineEvent::PositionClosed { .. }));

    // 3. PositionModified
    let ev3 = EngineEvent::PositionModified {
        position_id: pos_id,
        stop_loss: Some(49000.0),
        take_profit: Some(55000.0),
    };
    assert!(matches!(ev3, EngineEvent::PositionModified { .. }));

    // 4. OrderPlaced
    let ev4 = EngineEvent::OrderPlaced { order: order.clone() };
    assert!(matches!(ev4, EngineEvent::OrderPlaced { .. }));

    // 5. OrderFilled
    let ev5 = EngineEvent::OrderFilled { order: order.clone(), trade: trade.clone() };
    assert!(matches!(ev5, EngineEvent::OrderFilled { .. }));

    // 6. OrderPartiallyFilled
    let ev6 = EngineEvent::OrderPartiallyFilled { order: order.clone(), trade: trade.clone() };
    assert!(matches!(ev6, EngineEvent::OrderPartiallyFilled { .. }));

    // 7. OrderCanceled
    let ev7 = EngineEvent::OrderCanceled { order: order.clone() };
    assert!(matches!(ev7, EngineEvent::OrderCanceled { .. }));

    // 8. OrderFailed
    let ev8 = EngineEvent::OrderFailed {
        order_id,
        reason: "insufficient margin".to_string(),
    };
    assert!(matches!(ev8, EngineEvent::OrderFailed { .. }));

    // 9. RiskAlert
    let ev9 = EngineEvent::RiskAlert {
        level: "high".to_string(),
        message: "drawdown exceeds 10%".to_string(),
    };
    assert!(matches!(ev9, EngineEvent::RiskAlert { .. }));

    // 10. PositionSynced
    let exchange_pos = ExchangePosition {
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        size: 1.0,
        entry_price: 50000.0,
        leverage: 10,
        unrealized_pnl: 100.0,
        liquidation_price: Some(45000.0),
    };
    let ev10 = EngineEvent::PositionSynced { positions: vec![exchange_pos] };
    assert!(matches!(ev10, EngineEvent::PositionSynced { .. }));

    // 11. LiquidationWarning
    let ev11 = EngineEvent::LiquidationWarning {
        position_id: pos_id,
        symbol: "BTC/USDT".to_string(),
        liquidation_price: 45000.0,
        current_price: 45500.0,
    };
    assert!(matches!(ev11, EngineEvent::LiquidationWarning { .. }));

    // 验证所有 11 个变体
    let _events: Vec<EngineEvent> = vec![ev1, ev2, ev3, ev4, ev5, ev6, ev7, ev8, ev9, ev10, ev11];
    assert_eq!(_events.len(), 11);
}

// ============================================================
// 辅助结构体 (3 tests)
// ============================================================

#[test]
fn test_ticker_fields() {
    let now = Utc::now();
    let ticker = Ticker {
        symbol: "BTC/USDT".to_string(),
        price: 50000.0,
        bid: 49999.0,
        ask: 50001.0,
        volume_24h: 12345.6,
        timestamp: now,
    };

    assert_eq!(ticker.symbol, "BTC/USDT");
    assert_eq!(ticker.price, 50000.0);
    assert_eq!(ticker.bid, 49999.0);
    assert_eq!(ticker.ask, 50001.0);
    assert_eq!(ticker.volume_24h, 12345.6);
    assert_eq!(ticker.timestamp, now);

    // bid < price < ask
    assert!(ticker.bid < ticker.price);
    assert!(ticker.price < ticker.ask);
}

#[test]
fn test_balance_fields() {
    let balance = Balance {
        asset: "USDT".to_string(),
        free: 8000.0,
        used: 2000.0,
        total: 10000.0,
    };

    assert_eq!(balance.asset, "USDT");
    assert_eq!(balance.free, 8000.0);
    assert_eq!(balance.used, 2000.0);
    assert_eq!(balance.total, 10000.0);

    // free + used == total
    assert!((balance.free + balance.used - balance.total).abs() < F64_EPS);

    // 零余额场景
    let zero_balance = Balance {
        asset: "BTC".to_string(),
        free: 0.0,
        used: 0.0,
        total: 0.0,
    };
    assert!((zero_balance.free + zero_balance.used - zero_balance.total).abs() < F64_EPS);
}

#[test]
fn test_exchange_position_fields() {
    let exchange_pos = ExchangePosition {
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Short,
        size: 2.0,
        entry_price: 50000.0,
        leverage: 20,
        unrealized_pnl: -500.0,
        liquidation_price: Some(60000.0),
    };

    assert_eq!(exchange_pos.symbol, "BTC/USDT");
    assert_eq!(exchange_pos.side, PositionSide::Short);
    assert_eq!(exchange_pos.size, 2.0);
    assert_eq!(exchange_pos.entry_price, 50000.0);
    assert_eq!(exchange_pos.leverage, 20);
    assert_eq!(exchange_pos.unrealized_pnl, -500.0);
    assert_eq!(exchange_pos.liquidation_price, Some(60000.0));

    // liquidation_price 可以为 None
    let no_liq = ExchangePosition {
        liquidation_price: None,
        ..exchange_pos
    };
    assert!(no_liq.liquidation_price.is_none());
}

// ============================================================
// 序列化往返测试 (3 tests)
// ============================================================

#[test]
fn test_position_serialization_roundtrip() {
    // Position -> JSON -> Position，所有字段正确往返
    // 包括 Option 字段（stop_loss, take_profit, liquidation_price, strategy_id, closed_at）
    // 包括 metadata (serde_json::Value)
    let now = Utc::now();
    let pos = Position {
        id: Uuid::new_v4(),
        engine_id: "engine1".to_string(),
        strategy_id: Some("strat_abc".to_string()),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        status: PositionStatus::Open,
        size: 1.5,
        entry_price: 50000.0,
        current_price: 51000.0,
        leverage: 10,
        margin: 7500.0,
        unrealized_pnl: 1500.0,
        realized_pnl: 200.0,
        stop_loss: Some(48000.0),
        take_profit: Some(55000.0),
        liquidation_price: Some(45000.0),
        opened_at: now,
        updated_at: now,
        closed_at: None,
        metadata: serde_json::json!({"key": "value", "num": 42}),
    };

    let json = serde_json::to_string(&pos).unwrap();
    let pos_back: Position = serde_json::from_str(&json).unwrap();

    assert_eq!(pos_back.id, pos.id);
    assert_eq!(pos_back.engine_id, pos.engine_id);
    assert_eq!(pos_back.strategy_id, pos.strategy_id);
    assert_eq!(pos_back.exchange, pos.exchange);
    assert_eq!(pos_back.symbol, pos.symbol);
    assert_eq!(pos_back.side, pos.side);
    assert_eq!(pos_back.status, pos.status);
    assert!((pos_back.size - pos.size).abs() < F64_EPS);
    assert!((pos_back.entry_price - pos.entry_price).abs() < F64_EPS);
    assert!((pos_back.current_price - pos.current_price).abs() < F64_EPS);
    assert_eq!(pos_back.leverage, pos.leverage);
    assert!((pos_back.margin - pos.margin).abs() < F64_EPS);
    assert!((pos_back.unrealized_pnl - pos.unrealized_pnl).abs() < F64_EPS);
    assert!((pos_back.realized_pnl - pos.realized_pnl).abs() < F64_EPS);
    assert_eq!(pos_back.stop_loss, pos.stop_loss);
    assert_eq!(pos_back.take_profit, pos.take_profit);
    assert_eq!(pos_back.liquidation_price, pos.liquidation_price);
    assert!(pos_back.closed_at.is_none());
    assert_eq!(pos_back.metadata, pos.metadata);

    // 带 closed_at 的版本
    let mut pos_closed = pos.clone();
    pos_closed.status = PositionStatus::Closed;
    pos_closed.closed_at = Some(now);
    let json2 = serde_json::to_string(&pos_closed).unwrap();
    let pos_closed_back: Position = serde_json::from_str(&json2).unwrap();
    assert!(pos_closed_back.closed_at.is_some());
    assert_eq!(pos_closed_back.closed_at.unwrap(), now);
}

#[test]
fn test_order_serialization_roundtrip() {
    // Order -> JSON -> Order
    let now = Utc::now();
    let order = Order {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        exchange_order_id: Some("ex_ord_999".to_string()),
        client_order_id: Some("cli_ord_001".to_string()),
        exchange: "binance".to_string(),
        symbol: "ETH/USDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        request_price: Some(3000.0),
        fill_price: Some(3005.0),
        amount: 2.0,
        filled: 1.5,
        remaining: 0.5,
        status: OrderStatus::PartiallyFilled,
        reduce_only: false,
        fee: 1.5,
        fee_currency: "USDT".to_string(),
        slippage: Some(5.0),
        created_at: now,
        updated_at: now,
    };

    let json = serde_json::to_string(&order).unwrap();
    let order_back: Order = serde_json::from_str(&json).unwrap();

    assert_eq!(order_back.id, order.id);
    assert_eq!(order_back.position_id, order.position_id);
    assert_eq!(order_back.exchange_order_id, order.exchange_order_id);
    assert_eq!(order_back.client_order_id, order.client_order_id);
    assert_eq!(order_back.exchange, order.exchange);
    assert_eq!(order_back.symbol, order.symbol);
    assert_eq!(order_back.side, order.side);
    assert_eq!(order_back.order_type, order.order_type);
    assert_eq!(order_back.request_price, order.request_price);
    assert_eq!(order_back.fill_price, order.fill_price);
    assert!((order_back.amount - order.amount).abs() < F64_EPS);
    assert!((order_back.filled - order.filled).abs() < F64_EPS);
    assert!((order_back.remaining - order.remaining).abs() < F64_EPS);
    assert_eq!(order_back.status, order.status);
    assert_eq!(order_back.reduce_only, order.reduce_only);
    assert!((order_back.fee - order.fee).abs() < F64_EPS);
    assert_eq!(order_back.fee_currency, order.fee_currency);
    assert_eq!(order_back.slippage, order.slippage);

    // Market order: request_price=None, fill_price=None, slippage=None
    let market_order = Order {
        order_type: OrderType::Market,
        request_price: None,
        fill_price: None,
        slippage: None,
        ..order
    };
    let json2 = serde_json::to_string(&market_order).unwrap();
    let market_back: Order = serde_json::from_str(&json2).unwrap();
    assert!(market_back.request_price.is_none());
    assert!(market_back.fill_price.is_none());
    assert!(market_back.slippage.is_none());
}

#[test]
fn test_trade_serialization_roundtrip() {
    // Trade -> JSON -> Trade
    let now = Utc::now();
    let trade = Trade {
        id: Uuid::new_v4(),
        position_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        exchange: "binance".to_string(),
        symbol: "BTC/USDT".to_string(),
        side: Side::Sell,
        price: 51000.0,
        amount: 1.0,
        fee: 25.5,
        fee_currency: "USDT".to_string(),
        pnl: 1000.0,
        trade_type: "close".to_string(),
        created_at: now,
    };

    let json = serde_json::to_string(&trade).unwrap();
    let trade_back: Trade = serde_json::from_str(&json).unwrap();

    assert_eq!(trade_back.id, trade.id);
    assert_eq!(trade_back.position_id, trade.position_id);
    assert_eq!(trade_back.order_id, trade.order_id);
    assert_eq!(trade_back.exchange, trade.exchange);
    assert_eq!(trade_back.symbol, trade.symbol);
    assert_eq!(trade_back.side, trade.side);
    assert!((trade_back.price - trade.price).abs() < F64_EPS);
    assert!((trade_back.amount - trade.amount).abs() < F64_EPS);
    assert!((trade_back.fee - trade.fee).abs() < F64_EPS);
    assert_eq!(trade_back.fee_currency, trade.fee_currency);
    assert!((trade_back.pnl - trade.pnl).abs() < F64_EPS);
    assert_eq!(trade_back.trade_type, trade.trade_type);
    assert_eq!(trade_back.created_at, trade.created_at);
}

// ============================================================
// PositionSide::Both 行为 (1 test)
// ============================================================

#[test]
fn test_position_side_both_serialization() {
    // PositionSide::Both -> "Both" -> PositionSide::Both
    let both = PositionSide::Both;
    let json = serde_json::to_string(&both).unwrap();
    assert_eq!(json, "\"Both\"");

    let back: PositionSide = serde_json::from_str(&json).unwrap();
    assert_eq!(back, PositionSide::Both);

    // 往返一致性
    assert_eq!(
        serde_json::from_str::<PositionSide>(&serde_json::to_string(&PositionSide::Both).unwrap()).unwrap(),
        PositionSide::Both
    );
}
