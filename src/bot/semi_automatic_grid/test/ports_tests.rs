use super::common::*;
use crate::bot::semi_automatic_grid::ports::*;
use uuid::Uuid;
use chrono::Utc;

fn now_utc() -> chrono::DateTime<chrono::Utc> { Utc::now() }

// ── OrderSide ──

#[test]
fn grid_side_as_str() {
    assert_eq!(OrderSide::Buy.as_str(), "buy");
    assert_eq!(OrderSide::Sell.as_str(), "sell");
}

#[test]
fn grid_side_serialization_roundtrip() {
    let buy_json = serde_json::to_string(&OrderSide::Buy).unwrap();
    let sell_json = serde_json::to_string(&OrderSide::Sell).unwrap();
    let buy: OrderSide = serde_json::from_str(&buy_json).unwrap();
    let sell: OrderSide = serde_json::from_str(&sell_json).unwrap();
    assert_eq!(buy, OrderSide::Buy);
    assert_eq!(sell, OrderSide::Sell);
}

#[test]
fn grid_side_deserialize_invalid() {
    let result = serde_json::from_str::<OrderSide>(r#""Invalid""#);
    assert!(result.is_err());
}

#[test]
fn grid_side_equality() {
    assert_eq!(OrderSide::Buy, OrderSide::Buy);
    assert_eq!(OrderSide::Sell, OrderSide::Sell);
    assert_ne!(OrderSide::Buy, OrderSide::Sell);
}

#[test]
fn grid_side_copy_semantics() {
    let a = OrderSide::Buy;
    let b = a;
    assert_eq!(a, b);
}

// ── OrderCommand ──

#[test]
fn grid_order_command_place_buy() {
    let cmd = OrderCommand::PlaceOrder {
        symbol: "BTCUSDT".to_string(),
        side: OrderSide::Buy,
        amount: 0.001,
        price: Some(50000.0),
        reduce_only: false,
    client_order_id: None,
        position_side: None,
    };
    match &cmd {
        OrderCommand::PlaceOrder { symbol, side, amount, price, reduce_only, client_order_id: _, position_side: _ } => {
            assert_eq!(symbol, "BTCUSDT");
            assert_eq!(*side, OrderSide::Buy);
            assert!((amount - 0.001).abs() < f64::EPSILON);
            assert_eq!(*price, Some(50000.0));
            assert!(!reduce_only);
        }
        _ => panic!("Expected PlaceOrder"),
    }
}

#[test]
fn grid_order_command_place_sell_reduce_only() {
    let cmd = OrderCommand::PlaceOrder {
        symbol: "ETHUSDT".to_string(),
        side: OrderSide::Sell,
        amount: 0.01,
        price: None,
        reduce_only: true,
    client_order_id: None,
        position_side: None,
    };
    match &cmd {
        OrderCommand::PlaceOrder { side, price, reduce_only, .. } => {
            assert_eq!(*side, OrderSide::Sell);
            assert!(price.is_none());
            assert!(*reduce_only);
        }
        _ => panic!("Expected PlaceOrder"),
    }
}

#[test]
fn grid_order_command_place_zero_amount() {
    let cmd = OrderCommand::PlaceOrder {
        symbol: "BTCUSDT".to_string(),
        side: OrderSide::Buy,
        amount: 0.0,
        price: Some(50000.0),
        reduce_only: false,
    client_order_id: None,
        position_side: None,
    };
    match &cmd {
        OrderCommand::PlaceOrder { amount, .. } => assert!((amount).abs() < f64::EPSILON),
        _ => panic!("Expected PlaceOrder"),
    }
}

#[test]
fn grid_order_command_place_large_amount() {
    let cmd = OrderCommand::PlaceOrder {
        symbol: "BTCUSDT".to_string(),
        side: OrderSide::Buy,
        amount: 1000000.0,
        price: Some(1.0),
        reduce_only: false,
    client_order_id: None,
        position_side: None,
    };
    match &cmd {
        OrderCommand::PlaceOrder { amount, .. } => assert!((*amount - 1000000.0).abs() < f64::EPSILON),
        _ => panic!("Expected PlaceOrder"),
    }
}

#[test]
fn grid_order_command_cancel_all_with_symbol() {
    let cmd = OrderCommand::CancelAllOrders { symbol: Some("ETHUSDT".to_string()) };
    match &cmd {
        OrderCommand::CancelAllOrders { symbol } => assert_eq!(symbol, &Some("ETHUSDT".to_string())),
        _ => panic!("Expected CancelAllOrders"),
    }
}

#[test]
fn grid_order_command_cancel_all_no_symbol() {
    let cmd = OrderCommand::CancelAllOrders { symbol: None };
    match &cmd {
        OrderCommand::CancelAllOrders { symbol } => assert!(symbol.is_none()),
        _ => panic!("Expected CancelAllOrders"),
    }
}

// ── OrderInfo ──

#[test]
fn grid_order_info_construction() {
    let id = Uuid::new_v4();
    let info = OrderInfo {
        id,
        side: OrderSide::Buy,
        fill_price: Some(51000.0),
        request_price: Some(50000.0),
        filled: 0.001,
                symbol: "BTC/USDT".to_string(),
                client_order_id: None,
    };
    assert_eq!(info.id, id);
    assert_eq!(info.side, OrderSide::Buy);
    assert_eq!(info.fill_price, Some(51000.0));
    assert_eq!(info.request_price, Some(50000.0));
    assert!((info.filled - 0.001).abs() < f64::EPSILON);
}

#[test]
fn grid_order_info_no_prices() {
    let info = OrderInfo {
        id: Uuid::new_v4(),
        side: OrderSide::Sell,
        fill_price: None,
        request_price: None,
        filled: 0.0,
                symbol: "BTC/USDT".to_string(),
                client_order_id: None,
    };
    assert!(info.fill_price.is_none());
    assert!(info.request_price.is_none());
    assert!((info.filled).abs() < f64::EPSILON);
}

#[test]
fn grid_order_info_fill_price_only() {
    let info = OrderInfo {
        id: Uuid::new_v4(),
        side: OrderSide::Buy,
        fill_price: Some(50000.0),
        request_price: None,
        filled: 0.001,
                symbol: "BTC/USDT".to_string(),
                client_order_id: None,
    };
    assert!(info.fill_price.is_some());
    assert!(info.request_price.is_none());
}

#[test]
fn grid_order_info_request_price_only() {
    let info = OrderInfo {
        id: Uuid::new_v4(),
        side: OrderSide::Sell,
        fill_price: None,
        request_price: Some(52000.0),
        filled: 0.001,
                symbol: "BTC/USDT".to_string(),
                client_order_id: None,
    };
    assert!(info.fill_price.is_none());
    assert!(info.request_price.is_some());
}

#[test]
fn grid_order_info_zero_filled() {
    let info = OrderInfo {
        id: Uuid::new_v4(),
        side: OrderSide::Buy,
        fill_price: Some(50000.0),
        request_price: None,
        filled: 0.0,
                symbol: "BTC/USDT".to_string(),
                client_order_id: None,
    };
    assert!((info.filled).abs() < f64::EPSILON);
}

// ── OrderEvent ──

#[test]
fn grid_order_event_order_placed() {
    let id = Uuid::new_v4();
    let event = OrderEvent::OrderPlaced {
        order: OrderInfo {
            id, side: OrderSide::Buy, fill_price: Some(50000.0),
            request_price: None, filled: 0.0,
                symbol: "BTC/USDT".to_string(),
                client_order_id: None,
        },
    };
    match event {
        OrderEvent::OrderPlaced { order } => {
            assert_eq!(order.id, id);
            assert_eq!(order.side, OrderSide::Buy);
        }
        _ => panic!("Expected OrderPlaced"),
    }
}

#[test]
fn grid_order_event_order_filled() {
    let id = Uuid::new_v4();
    let event = OrderEvent::OrderFilled {
        order: OrderInfo {
            id, side: OrderSide::Sell, fill_price: Some(52000.0),
            request_price: Some(52260.0), filled: 0.001,
                symbol: "BTC/USDT".to_string(),
                client_order_id: None,
        },
    };
    match event {
        OrderEvent::OrderFilled { order } => {
            assert_eq!(order.id, id);
            assert_eq!(order.side, OrderSide::Sell);
            assert!((order.filled - 0.001).abs() < f64::EPSILON);
        }
        _ => panic!("Expected OrderFilled"),
    }
}

#[test]
fn grid_order_event_order_canceled() {
    let id = Uuid::new_v4();
    let event = OrderEvent::OrderCanceled { order_id: id, symbol: None };
    match event {
        OrderEvent::OrderCanceled { order_id, .. } => assert_eq!(order_id, id),
        _ => panic!("Expected OrderCanceled"),
    }
}

#[test]
fn grid_order_event_order_failed() {
    let id = Uuid::new_v4();
    let event = OrderEvent::OrderFailed { order_id: id, reason: "Insufficient margin".to_string() };
    match event {
        OrderEvent::OrderFailed { order_id, reason } => {
            assert_eq!(order_id, id);
            assert_eq!(reason, "Insufficient margin");
        }
        _ => panic!("Expected OrderFailed"),
    }
}

#[test]
fn grid_order_event_order_failed_empty_reason() {
    let id = Uuid::new_v4();
    let event = OrderEvent::OrderFailed { order_id: id, reason: String::new() };
    match event {
        OrderEvent::OrderFailed { reason, .. } => assert!(reason.is_empty()),
        _ => panic!("Expected OrderFailed"),
    }
}

#[test]
fn grid_order_event_risk_alert() {
    let event = OrderEvent::RiskAlert {
        level: "CloseAll".to_string(),
        message: "High exposure".to_string(),
    };
    match event {
        OrderEvent::RiskAlert { level, message } => {
            assert_eq!(level, "CloseAll");
            assert_eq!(message, "High exposure");
        }
        _ => panic!("Expected RiskAlert"),
    }
}

#[test]
fn grid_order_event_risk_alert_info_level() {
    let event = OrderEvent::RiskAlert {
        level: "Info".to_string(),
        message: "Just info".to_string(),
    };
    match event {
        OrderEvent::RiskAlert { level, .. } => assert_eq!(level, "Info"),
        _ => panic!("Expected RiskAlert"),
    }
}

#[test]
fn grid_order_event_liquidation_warning() {
    let event = OrderEvent::LiquidationWarning {
        symbol: "BTCUSDT".to_string(),
        liquidation_price: 45000.0,
        current_price: 46000.0,
    };
    match event {
        OrderEvent::LiquidationWarning { symbol, liquidation_price, current_price } => {
            assert_eq!(symbol, "BTCUSDT");
            assert!((liquidation_price - 45000.0).abs() < f64::EPSILON);
            assert!((current_price - 46000.0).abs() < f64::EPSILON);
        }
        _ => panic!("Expected LiquidationWarning"),
    }
}

#[test]
fn grid_order_event_liquidation_warning_close_to_liquidation() {
    let event = OrderEvent::LiquidationWarning {
        symbol: "ETHUSDT".to_string(),
        liquidation_price: 3000.0,
        current_price: 3001.0,
    };
    match event {
        OrderEvent::LiquidationWarning { current_price, liquidation_price, .. } => {
            assert!((current_price - liquidation_price - 1.0).abs() < f64::EPSILON);
        }
        _ => panic!("Expected LiquidationWarning"),
    }
}

// ── GridTradeRecord ──

#[test]
fn grid_trade_record_construction() {
    let record = GridTradeRecord {
        id: Uuid::new_v4(),
        grid_level: 3,
        open_side: "buy".to_string(),
        open_price: 50000.0,
        open_quantity: 0.001,
        close_side: None,
        close_price: None,
        close_quantity: None,
        pnl: 5.0,
        opened_at: now_utc(),
    };
    assert_eq!(record.grid_level, 3);
    assert_eq!(record.open_side, "buy");
    assert!((record.open_quantity - 0.001).abs() < f64::EPSILON);
    assert!((record.pnl - 5.0).abs() < f64::EPSILON);
}

#[test]
fn grid_trade_record_negative_pnl() {
    let record = GridTradeRecord {
        id: Uuid::new_v4(),
        grid_level: 5,
        open_side: "buy".to_string(),
        open_price: 50000.0,
        open_quantity: 0.002,
        close_side: Some("sell".to_string()),
        close_price: Some(49000.0),
        close_quantity: Some(0.002),
        pnl: -3.5,
        opened_at: now_utc(),
    };
    assert!(record.pnl < 0.0);
}

#[test]
fn grid_trade_record_zero_pnl() {
    let record = GridTradeRecord {
        id: Uuid::new_v4(),
        grid_level: 0,
        open_side: "buy".to_string(),
        open_price: 50000.0,
        open_quantity: 0.001,
        close_side: None,
        close_price: None,
        close_quantity: None,
        pnl: 0.0,
        opened_at: now_utc(),
    };
    assert!((record.pnl).abs() < f64::EPSILON);
}

#[test]
fn grid_trade_record_zero_quantity() {
    let record = GridTradeRecord {
        id: Uuid::new_v4(),
        grid_level: 2,
        open_side: "sell".to_string(),
        open_price: 0.0,
        open_quantity: 0.0,
        close_side: None,
        close_price: None,
        close_quantity: None,
        pnl: 0.0,
        opened_at: now_utc(),
    };
    assert!((record.open_quantity).abs() < f64::EPSILON);
}

#[test]
fn grid_trade_record_negative_level() {
    let record = GridTradeRecord {
        id: Uuid::new_v4(),
        grid_level: -1,
        open_side: "buy".to_string(),
        open_price: 50000.0,
        open_quantity: 0.001,
        close_side: None,
        close_price: None,
        close_quantity: None,
        pnl: 0.0,
        opened_at: now_utc(),
    };
    assert!(record.grid_level < 0);
}

// ── GridBotConfig ──

#[test]
fn grid_bot_config_construction() {
    let id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let config = GridBotConfig {
        id, user_id,
        name: "Test Bot".to_string(),
        symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(),
        grid_count: 10,
        upper_price: 60000.0,
        lower_price: 50000.0,
        grid_profit_pct: 0.5,
        quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: true,
        adjust_interval_secs: 300,
        market_regime: Some("trending".to_string()),
        grid_levels_json: None,
        system_prompt: Some("test prompt".to_string()),
        last_adjusted_at: None,
    };
    assert_eq!(config.id, id);
    assert!(config.dynamic_adjust);
    assert_eq!(config.market_regime, Some("trending".to_string()));
    assert_eq!(config.system_prompt, Some("test prompt".to_string()));
}

#[test]
fn grid_bot_config_zero_grid_count() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "Bad".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 0,
        upper_price: 60000.0, lower_price: 50000.0,
        grid_profit_pct: 0.5, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert_eq!(config.grid_count, 0);
}

#[test]
fn grid_bot_config_inverted_prices() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "Inverted".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 10,
        upper_price: 40000.0, lower_price: 50000.0,
        grid_profit_pct: 0.5, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!(config.upper_price < config.lower_price);
}

#[test]
fn grid_bot_config_zero_profit_pct() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "ZeroProfit".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 5,
        upper_price: 60000.0, lower_price: 50000.0,
        grid_profit_pct: 0.0, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!((config.grid_profit_pct).abs() < f64::EPSILON);
}

#[test]
fn grid_bot_config_zero_quantity() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "ZeroQty".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 5,
        upper_price: 60000.0, lower_price: 50000.0,
        grid_profit_pct: 0.5, quantity_per_grid: 0.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!((config.quantity_per_grid).abs() < f64::EPSILON);
}

#[test]
fn grid_bot_config_negative_profit_pct() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "NegProfit".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 5,
        upper_price: 60000.0, lower_price: 50000.0,
        grid_profit_pct: -1.0, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!(config.grid_profit_pct < 0.0);
}

#[test]
fn grid_bot_config_negative_grid_count() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "NegCount".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: -5,
        upper_price: 60000.0, lower_price: 50000.0,
        grid_profit_pct: 0.5, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!(config.grid_count < 0);
}

#[test]
fn grid_bot_config_no_optional_fields() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "Minimal".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 5,
        upper_price: 60000.0, lower_price: 50000.0,
        grid_profit_pct: 0.5, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!(config.market_regime.is_none());
    assert!(config.system_prompt.is_none());
}

#[test]
fn grid_bot_config_zero_prices() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "ZeroPrices".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 5,
        upper_price: 0.0, lower_price: 0.0,
        grid_profit_pct: 0.5, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!((config.upper_price).abs() < f64::EPSILON);
    assert!((config.lower_price).abs() < f64::EPSILON);
}

// ── Mock 实现 trait 验证 ──

#[tokio::test]
async fn mock_price_provider_valid_price() {
    let provider = MockPriceProvider::new(55000.0);
    let price = provider.get_price("binance", "BTCUSDT").await;
    assert_eq!(price, Some(55000.0));
}

#[tokio::test]
async fn mock_price_provider_zero_price_returns_none() {
    let provider = MockPriceProvider::new(0.0);
    let price = provider.get_price("binance", "BTCUSDT").await;
    assert!(price.is_none());
}

#[tokio::test]
async fn mock_price_provider_negative_price_returns_none() {
    let provider = MockPriceProvider::new(-100.0);
    let price = provider.get_price("binance", "BTCUSDT").await;
    assert!(price.is_none());
}

#[tokio::test]
async fn mock_order_executor_success() {
    let executor = MockOrderExecutor::new();
    let cmd = OrderCommand::PlaceOrder {
        symbol: "BTCUSDT".to_string(),
        side: OrderSide::Buy,
        amount: 0.001,
        price: Some(50000.0),
        reduce_only: false,
    client_order_id: None,
        position_side: None,
    };
    let result = executor.send_command(cmd).await;
    assert!(result.is_ok());
    let commands = executor.commands().await;
    assert_eq!(commands.len(), 1);
}

#[tokio::test]
async fn mock_order_executor_failure() {
    let executor = MockOrderExecutor::failing();
    let cmd = OrderCommand::CancelAllOrders { symbol: None };
    let result = executor.send_command(cmd).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn mock_store_load_trades_empty() {
    let store = MockWorkerStore::new();
    let trades = store.load_trades(Uuid::new_v4()).await.unwrap();
    assert!(trades.is_empty());
}

#[tokio::test]
async fn mock_store_load_trades_with_data() {
    let store = MockWorkerStore::new().with_trades(vec![
        GridTradeRecord { id: Uuid::new_v4(), grid_level: 0, open_side: "buy".to_string(), open_price: 50000.0, open_quantity: 0.001, close_side: None, close_price: None, close_quantity: None, pnl: 0.0, opened_at: now_utc() },
        GridTradeRecord { id: Uuid::new_v4(), grid_level: 1, open_side: "buy".to_string(), open_price: 50000.0, open_quantity: 0.001, close_side: Some("sell".to_string()), close_price: Some(51000.0), close_quantity: Some(0.001), pnl: 5.0, opened_at: now_utc() },
    ]);
    let trades = store.load_trades(Uuid::new_v4()).await.unwrap();
    assert_eq!(trades.len(), 2);
}

#[tokio::test]
async fn mock_store_failing_load() {
    let store = MockWorkerStore::failing();
    let result = store.load_trades(Uuid::new_v4()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn mock_store_record_open_trade() {
    let store = MockWorkerStore::new();
    let bot_id = Uuid::new_v4();
    let trade_id = store.record_open_trade(bot_id, Uuid::new_v4(), "BTCUSDT", "binance", 0, "buy", 50000.0, 0.001, None).await.unwrap();
    let recorded = store.open_trades.lock().await;
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].0, bot_id);
    assert_eq!(recorded[0].1, trade_id);
    assert_eq!(recorded[0].2, "buy");
    assert_eq!(recorded[0].3, 0);
}

#[tokio::test]
async fn mock_store_save_stats() {
    let store = MockWorkerStore::new();
    let bot_id = Uuid::new_v4();
    store.save_stats(bot_id, 100.0, 25.0, 10, 5, None).await.unwrap();
    let stats = store.stats_saved.lock().await;
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].0, bot_id);
    assert!((stats[0].1 - 100.0).abs() < f64::EPSILON);
    assert!((stats[0].2 - 25.0).abs() < f64::EPSILON);
    assert_eq!(stats[0].3, 10);
    assert_eq!(stats[0].4, 5);
}

#[tokio::test]
async fn mock_store_update_bot_status() {
    let store = MockWorkerStore::new();
    let bot_id = Uuid::new_v4();
    store.update_bot_status(bot_id, "running").await.unwrap();
    let statuses = store.statuses_updated.lock().await;
    assert!(statuses.contains(&(bot_id, "running".to_string())));
}

#[tokio::test]
async fn mock_store_update_grid_params() {
    let store = MockWorkerStore::new();
    let bot_id = Uuid::new_v4();
    store.update_grid_params(bot_id, 65000.0, 45000.0).await.unwrap();
    let params = store.grid_params_updated.lock().await;
    assert!(params.contains(&(bot_id, 65000.0, 45000.0)));
}

#[tokio::test]
async fn mock_store_delete_bot() {
    let store = MockWorkerStore::new();
    let bot_id = Uuid::new_v4();
    store.delete_bot(bot_id).await.unwrap();
    let deleted = store.deleted_bots.lock().await;
    assert!(deleted.contains(&bot_id));
}

// ── GridBotConfig 边界补充 ──

#[test]
fn grid_bot_config_same_upper_lower_price() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "Same".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 5,
        upper_price: 50000.0, lower_price: 50000.0,
        grid_profit_pct: 0.5, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!((config.upper_price - config.lower_price).abs() < f64::EPSILON);
}

#[test]
fn grid_bot_config_very_large_grid_count() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "LargeCount".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 10000,
        upper_price: 60000.0, lower_price: 50000.0,
        grid_profit_pct: 0.5, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert_eq!(config.grid_count, 10000);
}

#[test]
fn grid_bot_config_very_small_prices() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "SmallPrices".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 5,
        upper_price: 0.001, lower_price: 0.0001,
        grid_profit_pct: 0.5, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!(config.upper_price < 1.0);
    assert!(config.lower_price < 1.0);
}

#[test]
fn grid_bot_config_negative_prices() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "NegPrices".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 5,
        upper_price: -60000.0, lower_price: -50000.0,
        grid_profit_pct: 0.5, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!(config.upper_price < 0.0);
    assert!(config.lower_price < 0.0);
}

#[test]
fn grid_bot_config_negative_quantity() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "NegQty".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 5,
        upper_price: 60000.0, lower_price: 50000.0,
        grid_profit_pct: 0.5, quantity_per_grid: -100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!(config.quantity_per_grid < 0.0);
}

#[test]
fn grid_bot_config_large_profit_pct() {
    let config = GridBotConfig {
        id: Uuid::new_v4(), user_id: Uuid::new_v4(),
        name: "LargeProfit".to_string(), symbol: "BTCUSDT".to_string(),
        exchange: "binance".to_string(), grid_count: 5,
        upper_price: 60000.0, lower_price: 50000.0,
        grid_profit_pct: 100.0, quantity_per_grid: 100.0,
        leverage: 10,
        dynamic_adjust: false, adjust_interval_secs: 300,
        market_regime: None, grid_levels_json: None, system_prompt: None, last_adjusted_at: None,
    };
    assert!((config.grid_profit_pct - 100.0).abs() < f64::EPSILON);
}

// ── OrderCommand 边界补充 ──

#[test]
fn grid_order_command_place_negative_amount() {
    let cmd = OrderCommand::PlaceOrder {
        symbol: "BTCUSDT".to_string(),
        side: OrderSide::Sell,
        amount: -0.001,
        price: Some(50000.0),
        reduce_only: false,
    client_order_id: None,
        position_side: None,
    };
    match &cmd {
        OrderCommand::PlaceOrder { amount, .. } => assert!(*amount < 0.0),
        _ => panic!("Expected PlaceOrder"),
    }
}

#[test]
fn grid_order_command_place_negative_price() {
    let cmd = OrderCommand::PlaceOrder {
        symbol: "BTCUSDT".to_string(),
        side: OrderSide::Buy,
        amount: 0.001,
        price: Some(-50000.0),
        reduce_only: false,
    client_order_id: None,
        position_side: None,
    };
    match &cmd {
        OrderCommand::PlaceOrder { price, .. } => assert!(price.unwrap() < 0.0),
        _ => panic!("Expected PlaceOrder"),
    }
}

#[test]
fn grid_order_command_place_market_order_no_price() {
    let cmd = OrderCommand::PlaceOrder {
        symbol: "ETHUSDT".to_string(),
        side: OrderSide::Buy,
        amount: 1.0,
        price: None,
        reduce_only: false,
    client_order_id: None,
        position_side: None,
    };
    match &cmd {
        OrderCommand::PlaceOrder { price, reduce_only, .. } => {
            assert!(price.is_none());
            assert!(!reduce_only);
        }
        _ => panic!("Expected PlaceOrder"),
    }
}

// ── OrderInfo 边界补充 ──

#[test]
fn grid_order_info_negative_filled() {
    let info = OrderInfo {
        id: Uuid::new_v4(),
        side: OrderSide::Sell,
        fill_price: Some(50000.0),
        request_price: None,
        filled: -0.001,
                symbol: "BTC/USDT".to_string(),
                client_order_id: None,
    };
    assert!(info.filled < 0.0);
}

#[test]
fn grid_order_info_very_large_filled() {
    let info = OrderInfo {
        id: Uuid::new_v4(),
        side: OrderSide::Buy,
        fill_price: Some(50000.0),
        request_price: None,
        filled: 1e10,
                symbol: "BTC/USDT".to_string(),
                client_order_id: None,
    };
    assert!(info.filled > 1e9);
}

#[test]
fn grid_order_info_both_prices_equal() {
    let info = OrderInfo {
        id: Uuid::new_v4(),
        side: OrderSide::Buy,
        fill_price: Some(50000.0),
        request_price: Some(50000.0),
        filled: 0.001,
                symbol: "BTC/USDT".to_string(),
                client_order_id: None,
    };
    assert_eq!(info.fill_price, info.request_price);
}

// ── GridTradeRecord 边界补充 ──

#[test]
fn grid_trade_record_very_large_pnl() {
    let record = GridTradeRecord {
        id: Uuid::new_v4(),
        grid_level: 0,
        open_side: "sell".to_string(),
        open_price: 100000.0,
        open_quantity: 100.0,
        close_side: Some("buy".to_string()),
        close_price: Some(200000.0),
        close_quantity: Some(100.0),
        pnl: 1e8,
        opened_at: now_utc(),
    };
    assert!(record.pnl > 1e7);
}

#[test]
fn grid_trade_record_very_large_quantity() {
    let record = GridTradeRecord {
        id: Uuid::new_v4(),
        grid_level: 0,
        open_side: "buy".to_string(),
        open_price: 1.0,
        open_quantity: 1e6,
        close_side: None,
        close_price: None,
        close_quantity: None,
        pnl: 0.0,
        opened_at: now_utc(),
    };
    assert!(record.open_quantity > 1e5);
}

#[test]
fn grid_trade_record_invalid_side_string() {
    let record = GridTradeRecord {
        id: Uuid::new_v4(),
        grid_level: 0,
        open_side: "unknown".to_string(),
        open_price: 0.0,
        open_quantity: 0.001,
        close_side: None,
        close_price: None,
        close_quantity: None,
        pnl: 0.0,
        opened_at: now_utc(),
    };
    assert_eq!(record.open_side, "unknown");
}
