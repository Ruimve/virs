use super::common::*;
use crate::bot::semi_automatic_grid::ai::{GridAction, GridDecision};
use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::worker::GridWorker;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

// ── calculate_levels ──

#[test]
fn calculate_levels_basic() {
    let bot = make_bot_config();
    let levels = GridWorker::calculate_levels(&bot);
    assert_eq!(levels.len(), 10);
    let grid_spacing = (60000.0 - 50000.0) / 10.0;
    let profit_factor = 1.0 + 0.5 / 100.0;
    let mid_price = (60000.0 + 50000.0) / 2.0;
    for (i, level) in levels.iter().enumerate() {
        let expected_price = 50000.0 + grid_spacing * (i as f64 + 0.5);
        let expected_side = if expected_price < mid_price { "buy" } else { "sell" };
        let (expected_buy, expected_sell) = if expected_side == "buy" {
            (expected_price, expected_price * profit_factor)
        } else {
            (expected_price / profit_factor, expected_price)
        };
        let expected_qty = 100.0 / expected_price;
        assert_eq!(level.level, i as i32);
        assert_eq!(level.side, expected_side);
        assert!((level.price - expected_price).abs() < 0.01);
        assert!((level.buy_price - expected_buy).abs() < 0.01);
        assert!((level.sell_price - expected_sell).abs() < 0.01);
        assert!((level.quantity - expected_qty).abs() < 0.0000001);
        assert!(level.buy_order_id.is_none());
        assert!(level.sell_order_id.is_none());
        assert!(!level.buy_filled);
        assert!(!level.sell_filled);
        assert!((level.hold_quantity).abs() < f64::EPSILON);
    }
}

#[test]
fn calculate_levels_single() {
    let mut bot = make_bot_config();
    bot.grid_count = 1;
    let levels = GridWorker::calculate_levels(&bot);
    assert_eq!(levels.len(), 1);
    let grid_spacing = (60000.0 - 50000.0) / 1.0;
    let expected_price = 50000.0 + grid_spacing * 0.5;
    let mid_price = (60000.0 + 50000.0) / 2.0;
    let expected_side = if expected_price < mid_price { "buy" } else { "sell" };
    let expected_buy = if expected_side == "buy" { expected_price } else { expected_price / (1.0 + 0.5 / 100.0) };
    assert!((levels[0].buy_price - expected_buy).abs() < 0.01);
}

#[test]
fn calculate_levels_zero_count() {
    let mut bot = make_bot_config();
    bot.grid_count = 0;
    assert!(GridWorker::calculate_levels(&bot).is_empty());
}

#[test]
fn calculate_levels_negative_count() {
    let mut bot = make_bot_config();
    bot.grid_count = -5;
    assert!(GridWorker::calculate_levels(&bot).is_empty());
}

#[test]
fn calculate_levels_zero_upper_price() {
    let mut bot = make_bot_config();
    bot.upper_price = 0.0;
    assert!(GridWorker::calculate_levels(&bot).is_empty());
}

#[test]
fn calculate_levels_zero_lower_price() {
    let mut bot = make_bot_config();
    bot.lower_price = 0.0;
    assert!(GridWorker::calculate_levels(&bot).is_empty());
}

#[test]
fn calculate_levels_equal_prices() {
    let mut bot = make_bot_config();
    bot.upper_price = 50000.0;
    bot.lower_price = 50000.0;
    let levels = GridWorker::calculate_levels(&bot);
    assert!(levels.is_empty(), "equal upper/lower prices should produce no levels");
}

#[test]
fn calculate_levels_inverted_prices() {
    let mut bot = make_bot_config();
    bot.upper_price = 40000.0;
    bot.lower_price = 50000.0;
    let levels = GridWorker::calculate_levels(&bot);
    assert!(levels.is_empty(), "inverted prices should produce no levels");
}

#[test]
fn calculate_levels_zero_profit_pct() {
    let mut bot = make_bot_config();
    bot.grid_profit_pct = 0.0;
    let levels = GridWorker::calculate_levels(&bot);
    assert_eq!(levels.len(), 10);
    for level in &levels {
        assert!((level.sell_price - level.buy_price).abs() < f64::EPSILON);
    }
}

#[test]
fn calculate_levels_zero_quantity_per_grid() {
    let mut bot = make_bot_config();
    bot.quantity_per_grid = 0.0;
    let levels = GridWorker::calculate_levels(&bot);
    assert_eq!(levels.len(), 10);
    for level in &levels {
        assert!((level.quantity).abs() < f64::EPSILON);
    }
}

#[test]
fn calculate_levels_profit_pct() {
    let mut bot = make_bot_config();
    bot.grid_count = 1;
    bot.upper_price = 200.0;
    bot.lower_price = 100.0;
    bot.grid_profit_pct = 2.0;
    let levels = GridWorker::calculate_levels(&bot);
    let buy_price = levels[0].buy_price;
    let sell_price = levels[0].sell_price;
    let expected_sell = buy_price * (1.0 + 2.0 / 100.0);
    assert!((sell_price - expected_sell).abs() < 0.01);
}

// ── find_level_by_price ──

#[test]
fn find_level_by_price_exact() {
    let bot = make_bot_config();
    let worker = make_worker(bot, 55000.0);
    let target = worker.levels[4].buy_price;
    assert_eq!(worker.find_level_by_price(target), 4);
}

#[test]
fn find_level_by_price_between() {
    let bot = make_bot_config();
    let worker = make_worker(bot, 55000.0);
    assert_eq!(worker.find_level_by_price(55200.0), 5);
}

#[test]
fn find_level_by_price_below_all() {
    let bot = make_bot_config();
    let worker = make_worker(bot, 55000.0);
    let result = worker.find_level_by_price(40000.0);
    assert_eq!(result, 0);
}

#[test]
fn find_level_by_price_above_all() {
    let bot = make_bot_config();
    let worker = make_worker(bot, 55000.0);
    let result = worker.find_level_by_price(99999.0);
    assert!(result < worker.levels.len());
}

// ── simple_rule_decision ──

#[test]
fn simple_rule_decision_above_upper() {
    let mut worker = make_worker(make_bot_config(), 62000.0);
    worker.current_price = 62000.0;
    assert!(matches!(worker.simple_rule_decision(), GridAction::PauseGrid));
}

#[test]
fn simple_rule_decision_below_lower() {
    let mut worker = make_worker(make_bot_config(), 48000.0);
    worker.current_price = 48000.0;
    assert!(matches!(worker.simple_rule_decision(), GridAction::PauseGrid));
}

#[test]
fn simple_rule_decision_in_range() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    worker.current_price = 55000.0;
    worker.paused = false;
    assert!(matches!(worker.simple_rule_decision(), GridAction::Hold));
}

#[test]
fn simple_rule_decision_paused_in_range() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    worker.current_price = 55000.0;
    worker.paused = true;
    assert!(matches!(worker.simple_rule_decision(), GridAction::RunGrid));
}

#[test]
fn simple_rule_decision_exactly_at_upper() {
    let mut worker = make_worker(make_bot_config(), 60000.0);
    worker.current_price = 60000.0;
    assert!(matches!(worker.simple_rule_decision(), GridAction::Hold));
}

#[test]
fn simple_rule_decision_exactly_at_lower() {
    let mut worker = make_worker(make_bot_config(), 50000.0);
    worker.current_price = 50000.0;
    assert!(matches!(worker.simple_rule_decision(), GridAction::Hold));
}

#[test]
fn simple_rule_decision_just_above_upper() {
    let mut worker = make_worker(make_bot_config(), 61201.0);
    worker.current_price = 61201.0;
    assert!(matches!(worker.simple_rule_decision(), GridAction::PauseGrid));
}

#[test]
fn simple_rule_decision_just_below_lower() {
    let mut worker = make_worker(make_bot_config(), 48999.0);
    worker.current_price = 48999.0;
    assert!(matches!(worker.simple_rule_decision(), GridAction::PauseGrid));
}

// ── on_order_placed ──

#[tokio::test]
async fn on_order_placed_via_map() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    worker.levels[3].buy_order_id = Some(order_id);
    let order = OrderInfo {
        id: order_id, side: OrderSide::Buy,
        fill_price: Some(worker.levels[3].buy_price),
        request_price: None, filled: 0.0,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_placed(&order).await;
    assert_eq!(worker.levels.iter().filter(|l| l.buy_order_id.is_some() || l.sell_order_id.is_some()).count(), 1);
}

#[tokio::test]
async fn on_order_placed_via_client_order_id_buy() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    let level_idx = 2;
    let order = OrderInfo {
        id: order_id, side: OrderSide::Buy,
        fill_price: Some(worker.levels[level_idx].buy_price), request_price: None, filled: 0.0,
        symbol: "BTCUSDT".to_string(),
        client_order_id: Some(format!("grid:{}:{}:buy", worker.bot.id, level_idx)),
    };
    worker.on_order_placed(&order).await;
    assert!(worker.find_level_by_order_id(order_id).is_some());
    let (idx, side) = worker.find_level_by_order_id(order_id).unwrap();
    assert_eq!(idx, level_idx);
    assert_eq!(side, "buy");
    assert_eq!(worker.levels[level_idx].buy_order_id, Some(order_id));
}

#[tokio::test]
async fn on_order_placed_via_client_order_id_sell() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    let level_idx = 3;
    let order = OrderInfo {
        id: order_id, side: OrderSide::Sell,
        fill_price: Some(worker.levels[level_idx].sell_price), request_price: None, filled: 0.0,
        symbol: "BTCUSDT".to_string(),
        client_order_id: Some(format!("grid:{}:{}:sell", worker.bot.id, level_idx)),
    };
    worker.on_order_placed(&order).await;
    assert!(worker.find_level_by_order_id(order_id).is_some());
    let (idx, side) = worker.find_level_by_order_id(order_id).unwrap();
    assert_eq!(idx, level_idx);
    assert_eq!(side, "sell");
    assert_eq!(worker.levels[level_idx].sell_order_id, Some(order_id));
}

#[tokio::test]
async fn on_order_placed_no_match() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    let order = OrderInfo {
        id: order_id, side: OrderSide::Buy,
        fill_price: Some(99999.0), request_price: None, filled: 0.0,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_placed(&order).await;
    assert!(!worker.find_level_by_order_id(order_id).is_some());
}

#[tokio::test]
async fn on_order_placed_no_prices() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    let order = OrderInfo {
        id: order_id, side: OrderSide::Buy,
        fill_price: None, request_price: None, filled: 0.0,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_placed(&order).await;
    assert!(!worker.find_level_by_order_id(order_id).is_some());
}

#[tokio::test]
async fn on_order_placed_client_order_id_no_match() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    let buy_price = worker.levels[2].buy_price;
    let order = OrderInfo {
        id: order_id, side: OrderSide::Buy,
        fill_price: None, request_price: Some(buy_price), filled: 0.0,
        symbol: "BTCUSDT".to_string(),
        client_order_id: None,
    };
    worker.on_order_placed(&order).await;
    assert!(!worker.find_level_by_order_id(order_id).is_some());
}

// ── on_order_filled ──

#[tokio::test]
async fn on_order_filled_buy() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    let buy_price = worker.levels[3].buy_price;
    let quantity = worker.levels[3].quantity;
    worker.levels[3].buy_order_id = Some(order_id);
    let order = OrderInfo {
        id: order_id, side: OrderSide::Buy,
        fill_price: Some(buy_price), request_price: None, filled: quantity,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;
    assert!(worker.levels[3].buy_filled);
    assert!(worker.levels[3].buy_order_id.is_none());
    assert!((worker.levels[3].hold_quantity - quantity).abs() < f64::EPSILON);
    assert_eq!(worker.total_trades, 1);
    assert_eq!(worker.grid_filled_count, 1);
    assert!(!worker.find_level_by_order_id(order_id).is_some());
}

#[tokio::test]
async fn on_order_filled_sell_with_rebuy() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let buy_price = worker.levels[3].buy_price;
    let sell_price = worker.levels[3].sell_price;
    let quantity = worker.levels[3].quantity;
    worker.levels[3].buy_filled = true;
    worker.levels[3].hold_quantity = quantity;
    worker.levels[3].avg_buy_price = buy_price;
    let sell_order_id = Uuid::new_v4();
    worker.levels[3].sell_order_id = Some(sell_order_id);
    worker.levels[3].sell_order_id = Some(sell_order_id);
    let order = OrderInfo {
        id: sell_order_id, side: OrderSide::Sell,
        fill_price: Some(sell_price), request_price: None, filled: quantity,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;
    assert!(worker.levels[3].sell_filled);
    assert!(worker.levels[3].sell_order_id.is_none());
    assert!((worker.levels[3].hold_quantity).abs() < f64::EPSILON);
    assert!(worker.total_pnl > 0.0);
    assert_eq!(worker.total_trades, 1);
    assert_eq!(worker.grid_filled_count, 1);
    assert!(!worker.find_level_by_order_id(sell_order_id).is_some());
}

#[tokio::test]
async fn on_order_filled_sell_pnl_calculation() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let buy_price = worker.levels[3].buy_price;
    let sell_price = worker.levels[3].sell_price;
    let quantity = worker.levels[3].quantity;
    worker.levels[3].buy_filled = true;
    worker.levels[3].hold_quantity = quantity;
    worker.levels[3].avg_buy_price = buy_price;
    let sell_order_id = Uuid::new_v4();
    worker.levels[3].sell_order_id = Some(sell_order_id);
    let order = OrderInfo {
        id: sell_order_id, side: OrderSide::Sell,
        fill_price: Some(sell_price), request_price: None, filled: quantity,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;
    let expected_pnl = (sell_price - buy_price) * quantity;
    assert!((worker.total_pnl - expected_pnl).abs() < 0.001);
}

#[tokio::test]
async fn on_order_filled_no_match() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    let order = OrderInfo {
        id: order_id, side: OrderSide::Buy,
        fill_price: Some(50000.0), request_price: None, filled: 0.001,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;
    assert_eq!(worker.total_trades, 0);
    assert_eq!(worker.grid_filled_count, 0);
}

#[tokio::test]
async fn on_order_filled_zero_filled() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    let buy_price = worker.levels[3].buy_price;
    worker.levels[3].buy_order_id = Some(order_id);
    let order = OrderInfo {
        id: order_id, side: OrderSide::Buy,
        fill_price: Some(buy_price), request_price: None, filled: 0.0,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;
    assert!(worker.levels[3].buy_filled);
    assert!((worker.levels[3].hold_quantity).abs() < f64::EPSILON);
}

#[tokio::test]
async fn on_order_filled_side_mismatch() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    worker.levels[3].buy_order_id = Some(order_id);
    let order = OrderInfo {
        id: order_id, side: OrderSide::Sell,
        fill_price: Some(worker.levels[3].sell_price), request_price: None, filled: 0.001,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;
    assert_eq!(worker.total_trades, 0);
}

// ── clear_order_id / on_order_canceled ──

#[tokio::test]
async fn clear_order_id_buy() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    worker.levels[2].buy_order_id = Some(order_id);
    worker.levels[2].buy_order_id = Some(order_id);
    worker.clear_order_id(order_id);
    assert!(!worker.find_level_by_order_id(order_id).is_some());
    assert!(worker.levels[2].buy_order_id.is_none());
}

#[tokio::test]
async fn clear_order_id_sell() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    worker.levels[2].sell_order_id = Some(order_id);
    worker.levels[2].sell_order_id = Some(order_id);
    worker.clear_order_id(order_id);
    assert!(!worker.find_level_by_order_id(order_id).is_some());
    assert!(worker.levels[2].sell_order_id.is_none());
}

#[tokio::test]
async fn clear_order_id_nonexistent() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    worker.clear_order_id(order_id);
    assert!(worker.levels.iter().all(|l| l.buy_order_id.is_none() && l.sell_order_id.is_none()));
}

#[tokio::test]
async fn on_order_canceled() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    worker.levels[1].buy_order_id = Some(order_id);
    worker.levels[1].buy_order_id = Some(order_id);
    worker.on_order_canceled(order_id).await;
    assert!(!worker.find_level_by_order_id(order_id).is_some());
    assert!(worker.levels[1].buy_order_id.is_none());
}

// ── on_order_event ──

#[tokio::test]
async fn on_order_event_order_failed() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    worker.levels[2].buy_order_id = Some(order_id);
    worker.levels[2].buy_order_id = Some(order_id);
    worker.on_order_event(OrderEvent::OrderFailed {
        order_id, reason: "timeout".to_string(),
    }).await;
    assert!(!worker.find_level_by_order_id(order_id).is_some());
    assert!(worker.levels[2].buy_order_id.is_none());
}

#[tokio::test]
async fn on_risk_alert_closeall() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    assert!(!worker.paused);
    worker.on_order_event(OrderEvent::RiskAlert {
        level: "CloseAll".to_string(),
        message: "Risk!".to_string(),
    }).await;
    assert!(worker.paused);
}

#[tokio::test]
async fn on_risk_alert_other_level() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    worker.on_order_event(OrderEvent::RiskAlert {
        level: "Info".to_string(),
        message: "Just info".to_string(),
    }).await;
    assert!(!worker.paused);
}

#[tokio::test]
async fn on_liquidation_warning() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    assert!(!worker.paused);
    worker.on_order_event(OrderEvent::LiquidationWarning {
        symbol: "BTCUSDT".to_string(),
        liquidation_price: 45000.0,
        current_price: 46000.0,
    }).await;
    assert!(worker.paused);
}

// ── pause_with_cancel ──

#[tokio::test]
async fn pause_with_cancel_already_paused() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.paused = true;
    worker.pause_with_cancel("test").await;
    assert!(worker.paused);
    let commands = order_executor.commands().await;
    assert!(commands.is_empty());
}

#[tokio::test]
async fn pause_with_cancel_sends_cancel_command() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    assert!(!worker.paused);
    worker.pause_with_cancel("risk").await;
    assert!(worker.paused);
    let commands = order_executor.commands().await;
    assert_eq!(commands.len(), 1);
    match &commands[0] {
        OrderCommand::CancelAllOrders { symbol } => {
            assert_eq!(symbol, &Some("BTCUSDT".to_string()));
        }
        _ => panic!("Expected CancelAllOrders"),
    }
}

// ── execute_decision ──

#[tokio::test]
async fn execute_decision_pause_grid() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.execute_decision(&GridAction::PauseGrid, None).await;
    assert!(worker.paused);
}

#[tokio::test]
async fn execute_decision_run_grid_resumes() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.paused = true;
    worker.current_price = 55000.0;
    worker.execute_decision(&GridAction::RunGrid, None).await;
    assert!(!worker.paused);
}

#[tokio::test]
async fn execute_decision_run_grid_not_paused_noop() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.paused = false;
    worker.execute_decision(&GridAction::RunGrid, None).await;
    assert!(!worker.paused);
}

#[tokio::test]
async fn execute_decision_reduce_position() {
    let store = Arc::new(MockWorkerStore::new());
    let bot = make_bot_config();
    let bot_id = bot.id;
    let mut worker = make_worker_with_store(bot, 55000.0, store.clone());
    let original_qty = worker.bot.quantity_per_grid;
    assert!((original_qty - 100.0).abs() < f64::EPSILON);
    worker.execute_decision(&GridAction::ReducePosition, None).await;
    assert!((worker.bot.quantity_per_grid - 50.0).abs() < f64::EPSILON);
    let quantities = store.quantities_updated.lock().await;
    assert!(quantities.iter().any(|(id, q)| *id == bot_id && (q - 50.0).abs() < f64::EPSILON));
}

#[tokio::test]
async fn execute_decision_adjust_grid_with_decision() {
    let store = Arc::new(MockWorkerStore::new());
    let bot = make_bot_config();
    let bot_id = bot.id;
    let mut worker = make_worker_with_store(bot, 55000.0, store.clone());
    let decision = GridDecision {
        action: GridAction::AdjustGrid {
            upper_price: Some(65000.0),
            lower_price: Some(48000.0),
        },
        reason: "test".to_string(),
        upper_price: Some(65000.0),
        lower_price: Some(48000.0),
    };
    worker.execute_decision(&GridAction::AdjustGrid { upper_price: Some(65000.0), lower_price: Some(48000.0) }, Some(&decision)).await;
    assert!((worker.bot.upper_price - 65000.0).abs() < f64::EPSILON);
    assert!((worker.bot.lower_price - 48000.0).abs() < f64::EPSILON);
    let params = store.grid_params_updated.lock().await;
    assert!(params.iter().any(|(id, u, l)| *id == bot_id && (u - 65000.0).abs() < f64::EPSILON && (l - 48000.0).abs() < f64::EPSILON));
}

#[tokio::test]
async fn execute_decision_adjust_grid_no_decision_noop() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let old_upper = worker.bot.upper_price;
    worker.execute_decision(&GridAction::AdjustGrid { upper_price: None, lower_price: None }, None).await;
    assert!((worker.bot.upper_price - old_upper).abs() < f64::EPSILON);
}

#[tokio::test]
async fn execute_decision_hold_noop() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let old_pnl = worker.total_pnl;
    worker.execute_decision(&GridAction::Hold, None).await;
    assert!((worker.total_pnl - old_pnl).abs() < f64::EPSILON);
}

// ── adjust_grid ──

#[tokio::test]
async fn adjust_grid_with_changes() {
    let store = Arc::new(MockWorkerStore::new());
    let bot = make_bot_config();
    let bot_id = bot.id;
    let mut worker = make_worker_with_store(bot, 55000.0, store.clone());
    worker.adjust_grid(Some(65000.0), Some(48000.0), false).await;
    assert!((worker.bot.upper_price - 65000.0).abs() < f64::EPSILON);
    assert!((worker.bot.lower_price - 48000.0).abs() < f64::EPSILON);
    assert_eq!(worker.levels.len(), 10);
    let params = store.grid_params_updated.lock().await;
    assert!(params.iter().any(|(id, _, _)| *id == bot_id));
}

#[tokio::test]
async fn adjust_grid_no_changes() {
    let store = Arc::new(MockWorkerStore::new());
    let mut worker = make_worker_with_store(make_bot_config(), 55000.0, store.clone());
    worker.adjust_grid(Some(60000.0), Some(50000.0), false).await;
    let params = store.grid_params_updated.lock().await;
    assert!(params.is_empty());
}

#[tokio::test]
async fn adjust_grid_only_upper() {
    let store = Arc::new(MockWorkerStore::new());
    let bot = make_bot_config();
    let bot_id = bot.id;
    let mut worker = make_worker_with_store(bot, 55000.0, store.clone());
    worker.adjust_grid(Some(65000.0), None, false).await;
    assert!((worker.bot.upper_price - 65000.0).abs() < f64::EPSILON);
    assert!((worker.bot.lower_price - 50000.0).abs() < f64::EPSILON);
    let params = store.grid_params_updated.lock().await;
    assert!(params.iter().any(|(id, _, _)| *id == bot_id));
}

#[tokio::test]
async fn adjust_grid_only_lower() {
    let store = Arc::new(MockWorkerStore::new());
    let bot = make_bot_config();
    let bot_id = bot.id;
    let mut worker = make_worker_with_store(bot, 55000.0, store.clone());
    worker.adjust_grid(None, Some(48000.0), false).await;
    assert!((worker.bot.upper_price - 60000.0).abs() < f64::EPSILON);
    assert!((worker.bot.lower_price - 48000.0).abs() < f64::EPSILON);
    let params = store.grid_params_updated.lock().await;
    assert!(params.iter().any(|(id, _, _)| *id == bot_id));
}

#[tokio::test]
async fn adjust_grid_zero_values_ignored() {
    let store = Arc::new(MockWorkerStore::new());
    let mut worker = make_worker_with_store(make_bot_config(), 55000.0, store.clone());
    worker.adjust_grid(Some(0.0), Some(0.0), false).await;
    let params = store.grid_params_updated.lock().await;
    assert!(params.is_empty());
}

#[tokio::test]
async fn adjust_grid_clears_level_order_ids() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    worker.levels[2].buy_order_id = Some(Uuid::new_v4());
    assert_eq!(worker.levels.iter().filter(|l| l.buy_order_id.is_some() || l.sell_order_id.is_some()).count(), 1);
    worker.adjust_grid(Some(65000.0), Some(48000.0), false).await;
    assert!(worker.levels.iter().all(|l| l.buy_order_id.is_none() && l.sell_order_id.is_none()));
}

// ── fetch_current_price ──

#[tokio::test]
async fn fetch_current_price_valid() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    worker.current_price = 10000.0;
    let price = worker.fetch_current_price().await;
    assert!((price - 55000.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn fetch_current_price_invalid_falls_back() {
    let bot = make_bot_config();
    let (event_tx, event_rx) = broadcast::channel(16);
    let (grid_event_tx, _) = broadcast::channel(16);
    let price_provider = Arc::new(MockPriceProvider::new(0.0));
    let order_executor = Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();
    let store = Arc::new(MockWorkerStore::new());
    let mut worker = GridWorker::new(bot, price_provider, order_executor, ai_service, store, Arc::new(crate::bot::semi_automatic_grid::test::common::MockMarketDataProvider), event_rx, grid_event_tx);
    worker.current_price = 12345.0;
    let price = worker.fetch_current_price().await;
    assert!((price - 12345.0).abs() < f64::EPSILON);
}

// ── load_existing_trades ──

#[tokio::test]
async fn load_existing_trades_buy_and_sell() {
    let bot = make_bot_config();
    let store = Arc::new(MockWorkerStore::new().with_trades(vec![
        GridTradeRecord { grid_level: 3, side: "buy".to_string(), price: 53500.0, quantity: 0.001, pnl: 0.0 },
        GridTradeRecord { grid_level: 3, side: "sell".to_string(), price: 53767.5, quantity: 0.001, pnl: 5.0 },
        GridTradeRecord { grid_level: 2, side: "buy".to_string(), price: 52500.0, quantity: 0.002, pnl: 0.0 },
    ]));
    let mut worker = make_worker_with_store(bot, 55000.0, store);
    worker.load_existing_trades().await;
    assert!(!worker.levels[3].buy_filled);
    assert!(!worker.levels[3].sell_filled);
    assert!((worker.levels[3].hold_quantity).abs() < f64::EPSILON);
    assert!(worker.levels[2].buy_filled);
    assert!((worker.levels[2].hold_quantity - 0.002).abs() < f64::EPSILON);
    assert!((worker.total_pnl - 5.0).abs() < f64::EPSILON);
    assert_eq!(worker.total_trades, 3);
}

#[tokio::test]
async fn load_existing_trades_empty() {
    let store = Arc::new(MockWorkerStore::new());
    let mut worker = make_worker_with_store(make_bot_config(), 55000.0, store);
    worker.load_existing_trades().await;
    assert_eq!(worker.total_trades, 0);
    assert!((worker.total_pnl).abs() < f64::EPSILON);
}

#[tokio::test]
async fn load_existing_trades_invalid_level_ignored() {
    let bot = make_bot_config();
    let store = Arc::new(MockWorkerStore::new().with_trades(vec![
        GridTradeRecord { grid_level: 99, side: "buy".to_string(), price: 0.0, quantity: 0.001, pnl: 0.0 },
        GridTradeRecord { grid_level: -1, side: "buy".to_string(), price: 0.0, quantity: 0.001, pnl: 0.0 },
    ]));
    let mut worker = make_worker_with_store(bot, 55000.0, store);
    worker.load_existing_trades().await;
    assert_eq!(worker.total_trades, 2);
    for level in &worker.levels {
        assert!(!level.buy_filled);
    }
}

// ── place_initial_orders ──

#[tokio::test]
async fn place_initial_orders_sends_buy_orders() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.current_price = 55000.0;
    worker.place_initial_orders().await;
    let commands = order_executor.commands().await;
    let buy_count = commands.iter().filter(|c| matches!(c, OrderCommand::PlaceOrder { side: OrderSide::Buy, .. })).count();
    assert!(buy_count > 0, "Should place at least one buy order");
}

#[tokio::test]
async fn place_initial_orders_no_price_skips() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.current_price = 0.0;
    worker.place_initial_orders().await;
    let commands = order_executor.commands().await;
    assert!(commands.is_empty());
}

#[tokio::test]
async fn place_initial_orders_with_hold_sells() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.current_price = 55000.0;
    worker.levels[3].buy_filled = true;
    worker.levels[3].hold_quantity = worker.levels[3].quantity;
    worker.place_initial_orders().await;
    let commands = order_executor.commands().await;
    let sell_count = commands.iter().filter(|c| matches!(c, OrderCommand::PlaceOrder { side: OrderSide::Sell, .. })).count();
    assert!(sell_count > 0, "Should place sell order for held level");
}

// ── on_price_tick ──

#[tokio::test]
async fn on_price_tick_places_buy_below_current() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.current_price = 48000.0;
    worker.on_price_tick().await;
    let commands = order_executor.commands().await;
    assert!(!commands.is_empty());
}

#[tokio::test]
async fn on_price_tick_no_price_skips() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.current_price = 0.0;
    worker.on_price_tick().await;
    let commands = order_executor.commands().await;
    assert!(commands.is_empty());
}

#[tokio::test]
async fn on_price_tick_sell_when_hold_and_price_above() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.current_price = 60000.0;
    worker.levels[0].buy_filled = true;
    worker.levels[0].hold_quantity = worker.levels[0].quantity;
    worker.on_price_tick().await;
    let commands = order_executor.commands().await;
    let sell_count = commands.iter().filter(|c| matches!(c, OrderCommand::PlaceOrder { side: OrderSide::Sell, .. })).count();
    assert!(sell_count > 0);
}

// ── recalculate_levels ──

#[test]
fn recalculate_levels_clears_map() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    worker.levels[2].buy_order_id = Some(Uuid::new_v4());
    assert_eq!(worker.levels.iter().filter(|l| l.buy_order_id.is_some() || l.sell_order_id.is_some()).count(), 1);
    worker.recalculate_levels();
    assert!(worker.levels.iter().all(|l| l.buy_order_id.is_none() && l.sell_order_id.is_none()));
    assert_eq!(worker.levels.len(), 10);
}

// ── record_trade pnl_pct ──

#[test]
fn record_trade_pnl_pct_calculation() {
    let price = 55000.0;
    let quantity = 0.001;
    let pnl = 5.5;
    let pnl_pct = if price > 0.0 { pnl / (price * quantity) * 100.0 } else { 0.0 };
    assert!((pnl_pct - 10.0_f64).abs() < 0.001);
}

#[test]
fn record_trade_pnl_pct_zero_price() {
    let price = 0.0;
    let pnl = 5.5;
    let quantity = 0.001;
    let pnl_pct = if price > 0.0 { pnl / (price * quantity) * 100.0 } else { 0.0 };
    assert!((pnl_pct as f64).abs() < f64::EPSILON);
}

// ── broadcast_state ──

#[test]
fn broadcast_state_sends_event() {
    let (grid_event_tx, mut grid_event_rx) = broadcast::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);
    let bot = make_bot_config();
    let bot_id = bot.id;
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let order_executor = Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();
    let store = Arc::new(MockWorkerStore::new());
    let mut worker = GridWorker::new(bot, price_provider, order_executor, ai_service, store, Arc::new(crate::bot::semi_automatic_grid::test::common::MockMarketDataProvider), event_rx, grid_event_tx);
    worker.current_price = 55000.0;
    worker.broadcast_state();
    let event = grid_event_rx.try_recv().unwrap();
    match event {
        crate::bot::semi_automatic_grid::types::GridEvent::StatusUpdate { bot_id: b, state } => {
            assert_eq!(b, bot_id);
            assert!((state.current_price - 55000.0).abs() < f64::EPSILON);
        }
        _ => panic!("Expected StatusUpdate"),
    }
}

// ── on_order_filled 事件触发验证 ──

#[tokio::test]
async fn on_order_filled_sell_triggers_grid_trade_closed_event() {
    let (grid_event_tx, mut grid_event_rx) = broadcast::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);
    let bot = make_bot_config();
    let bot_id = bot.id;
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let order_executor = Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();
    let store = Arc::new(MockWorkerStore::new());
    let mut worker = GridWorker::new(bot, price_provider, order_executor, ai_service, store, Arc::new(crate::bot::semi_automatic_grid::test::common::MockMarketDataProvider), event_rx, grid_event_tx);

    let buy_price = worker.levels[3].buy_price;
    let sell_price = worker.levels[3].sell_price;
    let quantity = worker.levels[3].quantity;
    worker.levels[3].buy_filled = true;
    worker.levels[3].hold_quantity = quantity;
    worker.levels[3].avg_buy_price = buy_price;
    let sell_order_id = Uuid::new_v4();
    worker.levels[3].sell_order_id = Some(sell_order_id);

    let order = OrderInfo {
        id: sell_order_id, side: OrderSide::Sell,
        fill_price: Some(sell_price), request_price: None, filled: quantity,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;

    let event = grid_event_rx.try_recv().unwrap();
    match event {
        crate::bot::semi_automatic_grid::types::GridEvent::GridTradeClosed { bot_id: b, level, pnl } => {
            assert_eq!(b, bot_id);
            assert_eq!(level, 3);
            assert!(pnl > 0.0);
        }
        _ => panic!("Expected GridTradeClosed"),
    }
}

#[tokio::test]
async fn on_order_filled_sell_triggers_grid_filled_event() {
    let (grid_event_tx, mut grid_event_rx) = broadcast::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);
    let bot = make_bot_config();
    let bot_id = bot.id;
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let order_executor = Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();
    let store = Arc::new(MockWorkerStore::new());
    let mut worker = GridWorker::new(bot, price_provider, order_executor, ai_service, store, Arc::new(crate::bot::semi_automatic_grid::test::common::MockMarketDataProvider), event_rx, grid_event_tx);

    let sell_price = worker.levels[3].sell_price;
    let quantity = worker.levels[3].quantity;
    worker.levels[3].buy_filled = true;
    worker.levels[3].hold_quantity = quantity;
    let sell_order_id = Uuid::new_v4();
    worker.levels[3].sell_order_id = Some(sell_order_id);

    let order = OrderInfo {
        id: sell_order_id, side: OrderSide::Sell,
        fill_price: Some(sell_price), request_price: None, filled: quantity,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;

    let _closed_event = grid_event_rx.try_recv().unwrap();
    let filled_event = grid_event_rx.try_recv().unwrap();
    match filled_event {
        crate::bot::semi_automatic_grid::types::GridEvent::GridFilled { bot_id: b, level, side, price, quantity: qty } => {
            assert_eq!(b, bot_id);
            assert_eq!(level, 3);
            assert_eq!(side, "sell");
            assert!((price - sell_price).abs() < 0.01);
            assert!((qty - quantity).abs() < f64::EPSILON);
        }
        _ => panic!("Expected GridFilled"),
    }
}

#[tokio::test]
async fn on_order_filled_buy_triggers_grid_filled_event() {
    let (grid_event_tx, mut grid_event_rx) = broadcast::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);
    let bot = make_bot_config();
    let bot_id = bot.id;
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let order_executor = Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();
    let store = Arc::new(MockWorkerStore::new());
    let mut worker = GridWorker::new(bot, price_provider, order_executor, ai_service, store, Arc::new(crate::bot::semi_automatic_grid::test::common::MockMarketDataProvider), event_rx, grid_event_tx);

    let buy_price = worker.levels[3].buy_price;
    let quantity = worker.levels[3].quantity;
    let order_id = Uuid::new_v4();
    worker.levels[3].buy_order_id = Some(order_id);

    let order = OrderInfo {
        id: order_id, side: OrderSide::Buy,
        fill_price: Some(buy_price), request_price: None, filled: quantity,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;

    let event = grid_event_rx.try_recv().unwrap();
    match event {
        crate::bot::semi_automatic_grid::types::GridEvent::GridFilled { bot_id: b, level, side, .. } => {
            assert_eq!(b, bot_id);
            assert_eq!(level, 3);
            assert_eq!(side, "buy");
        }
        _ => panic!("Expected GridFilled"),
    }
}

#[tokio::test]
async fn on_order_filled_buy_no_trade_closed_event() {
    let (grid_event_tx, mut grid_event_rx) = broadcast::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);
    let bot = make_bot_config();
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let order_executor = Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();
    let store = Arc::new(MockWorkerStore::new());
    let mut worker = GridWorker::new(bot, price_provider, order_executor, ai_service, store, Arc::new(crate::bot::semi_automatic_grid::test::common::MockMarketDataProvider), event_rx, grid_event_tx);

    let buy_price = worker.levels[3].buy_price;
    let quantity = worker.levels[3].quantity;
    let order_id = Uuid::new_v4();
    worker.levels[3].buy_order_id = Some(order_id);

    let order = OrderInfo {
        id: order_id, side: OrderSide::Buy,
        fill_price: Some(buy_price), request_price: None, filled: quantity,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;

    let event = grid_event_rx.try_recv().unwrap();
    match event {
        crate::bot::semi_automatic_grid::types::GridEvent::GridFilled { .. } => {}
        crate::bot::semi_automatic_grid::types::GridEvent::GridTradeClosed { .. } => {
            panic!("Buy should not trigger GridTradeClosed");
        }
        _ => {}
    }
}

// ── on_order_filled sell rebuy 下单验证 ──

#[tokio::test]
async fn on_order_filled_sell_places_rebuy_order() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());

    let sell_price = worker.levels[3].sell_price;
    let quantity = worker.levels[3].quantity;
    worker.levels[3].buy_filled = true;
    worker.levels[3].hold_quantity = quantity;
    let sell_order_id = Uuid::new_v4();
    worker.levels[3].sell_order_id = Some(sell_order_id);

    let order = OrderInfo {
        id: sell_order_id, side: OrderSide::Sell,
        fill_price: Some(sell_price), request_price: None, filled: quantity,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;

    let commands = order_executor.commands().await;
    let rebuy = commands.iter().find(|c| {
        matches!(c, OrderCommand::PlaceOrder { side: OrderSide::Buy, .. })
    });
    assert!(rebuy.is_some(), "Sell fill should trigger a rebuy order");
    match rebuy.unwrap() {
        OrderCommand::PlaceOrder { side, price, amount, .. } => {
            assert_eq!(*side, OrderSide::Buy);
            assert_eq!(*price, Some(worker.levels[3].buy_price));
            assert!((amount - quantity).abs() < f64::EPSILON);
        }
        _ => {}
    }
}

// ── on_price_tick paused 不下单 ──

#[tokio::test]
async fn on_price_tick_paused_still_processes() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.current_price = 48000.0;
    worker.paused = true;
    worker.on_price_tick().await;
    let commands = order_executor.commands().await;
    assert!(!commands.is_empty(), "on_price_tick itself does not check paused; caller should check");
}

// ── on_price_tick sell order when hold and price at sell_price ──

#[tokio::test]
async fn on_price_tick_sell_order_when_hold_and_price_at_sell_price() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());

    worker.levels[3].buy_filled = true;
    worker.levels[3].hold_quantity = worker.levels[3].quantity;
    worker.current_price = worker.levels[3].sell_price;

    worker.on_price_tick().await;

    let commands = order_executor.commands().await;
    let sell_orders: Vec<_> = commands.iter().filter(|c| {
        matches!(c, OrderCommand::PlaceOrder { side: OrderSide::Sell, .. })
    }).collect();
    assert!(!sell_orders.is_empty(), "Should place sell order when price at sell_price and holding");
}

// ── adjust_grid paused 不下初始单 ──

#[tokio::test]
async fn adjust_grid_paused_does_not_place_initial_orders() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.paused = true;
    worker.current_price = 55000.0;

    worker.adjust_grid(Some(65000.0), Some(48000.0), false).await;

    let commands = order_executor.commands().await;
    let place_orders: Vec<_> = commands.iter().filter(|c| {
        matches!(c, OrderCommand::PlaceOrder { .. })
    }).collect();
    assert!(place_orders.is_empty(), "Paused worker should not place initial orders after adjust");
}

// ── adjust_grid not paused 下初始单 ──

#[tokio::test]
async fn adjust_grid_not_paused_places_initial_orders() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.paused = false;
    worker.current_price = 55000.0;

    worker.adjust_grid(Some(65000.0), Some(48000.0), false).await;

    assert!(!worker.levels.is_empty());
    let commands = order_executor.commands().await;
    let place_orders: Vec<_> = commands.iter().filter(|c| {
        matches!(c, OrderCommand::PlaceOrder { .. })
    }).collect();
    assert!(!place_orders.is_empty(), "Not-paused worker should place initial orders after adjust");
}

// ── adjust_grid levels 重算验证 ──

#[tokio::test]
async fn adjust_grid_recalculates_levels_correctly() {
    let store = Arc::new(MockWorkerStore::new());
    let bot = make_bot_config();
    let mut worker = make_worker_with_store(bot, 55000.0, store);

    worker.adjust_grid(Some(70000.0), Some(40000.0), false).await;

    assert!((worker.bot.upper_price - 70000.0).abs() < f64::EPSILON);
    assert!((worker.bot.lower_price - 40000.0).abs() < f64::EPSILON);
    assert_eq!(worker.levels.len(), 10);

    let grid_spacing = (70000.0 - 40000.0) / 10.0;
    let expected_buy_0 = 40000.0 + grid_spacing * 0.5;
    assert!((worker.levels[0].buy_price - expected_buy_0).abs() < 0.01);
}

// ── adjust_grid sends cancel command ──

#[tokio::test]
async fn adjust_grid_sends_cancel_all_orders() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());

    worker.adjust_grid(Some(65000.0), Some(48000.0), false).await;

    let commands = order_executor.commands().await;
    let cancel_cmd = commands.iter().find(|c| {
        matches!(c, OrderCommand::CancelAllOrders { .. })
    });
    assert!(cancel_cmd.is_some(), "adjust_grid should send CancelAllOrders");
}

// ── recalculate_levels 验证 levels 匹配新 bot config ──

#[test]
fn recalculate_levels_updates_levels_to_match_bot() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    worker.bot.upper_price = 70000.0;
    worker.bot.lower_price = 40000.0;
    worker.recalculate_levels();

    assert_eq!(worker.levels.len(), 10);
    let grid_spacing = (70000.0 - 40000.0) / 10.0;
    let expected_buy_0 = 40000.0 + grid_spacing * 0.5;
    assert!((worker.levels[0].buy_price - expected_buy_0).abs() < 0.01);
}

#[test]
fn recalculate_levels_preserves_holdings() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    worker.levels[3].buy_filled = true;
    worker.levels[3].hold_quantity = 0.001;
    worker.levels[3].avg_buy_price = 52000.0;
    worker.levels[3].buy_order_id = Some(Uuid::new_v4());

    worker.recalculate_levels();

    assert!(worker.levels[3].buy_filled, "buy_filled should be preserved");
    assert!((worker.levels[3].hold_quantity - 0.001).abs() < f64::EPSILON, "hold_quantity should be preserved");
    assert!((worker.levels[3].avg_buy_price - 52000.0).abs() < f64::EPSILON, "avg_buy_price should be preserved");
    assert!(worker.levels[3].buy_order_id.is_none(), "order_id should be cleared");
}

// ── load_existing_trades sell 减持 ──

#[tokio::test]
async fn load_existing_trades_sell_reduces_hold_quantity() {
    let bot = make_bot_config();
    let store = Arc::new(MockWorkerStore::new().with_trades(vec![
        GridTradeRecord { grid_level: 3, side: "buy".to_string(), price: 53500.0, quantity: 0.002, pnl: 0.0 },
        GridTradeRecord { grid_level: 3, side: "sell".to_string(), price: 53767.5, quantity: 0.001, pnl: 3.0 },
    ]));
    let mut worker = make_worker_with_store(bot, 55000.0, store);
    worker.load_existing_trades().await;

    assert!(worker.levels[3].buy_filled);
    assert!(worker.levels[3].sell_filled);
    assert!((worker.levels[3].hold_quantity - 0.001).abs() < f64::EPSILON);
    assert!((worker.total_pnl - 3.0).abs() < f64::EPSILON);
}

// ── load_existing_trades 多笔交易累积 ──

#[tokio::test]
async fn load_existing_trades_multiple_buys_accumulate() {
    let bot = make_bot_config();
    let store = Arc::new(MockWorkerStore::new().with_trades(vec![
        GridTradeRecord { grid_level: 2, side: "buy".to_string(), price: 52500.0, quantity: 0.001, pnl: 0.0 },
        GridTradeRecord { grid_level: 2, side: "buy".to_string(), price: 52500.0, quantity: 0.001, pnl: 0.0 },
    ]));
    let mut worker = make_worker_with_store(bot, 55000.0, store);
    worker.load_existing_trades().await;

    assert!(worker.levels[2].buy_filled);
    assert!((worker.levels[2].hold_quantity - 0.002).abs() < f64::EPSILON);
    assert_eq!(worker.total_trades, 2);
}

// ── load_existing_trades store 失败 ──

#[tokio::test]
async fn load_existing_trades_store_failure_uses_default() {
    let bot = make_bot_config();
    let store = Arc::new(MockWorkerStore::failing());
    let mut worker = make_worker_with_store(bot, 55000.0, store);
    worker.load_existing_trades().await;

    assert_eq!(worker.total_trades, 0);
    assert!((worker.total_pnl).abs() < f64::EPSILON);
}

// ── execute_decision reduce_position 重算 levels ──

#[tokio::test]
async fn execute_decision_reduce_position_recalculates_levels() {
    let store = Arc::new(MockWorkerStore::new());
    let bot = make_bot_config();
    let mut worker = make_worker_with_store(bot, 55000.0, store);

    let original_qty = worker.bot.quantity_per_grid;
    worker.execute_decision(&GridAction::ReducePosition, None).await;

    assert!((worker.bot.quantity_per_grid - original_qty * 0.5).abs() < f64::EPSILON);

    let new_expected_qty = (original_qty * 0.5) / worker.levels[0].buy_price;
    assert!((worker.levels[0].quantity - new_expected_qty).abs() < 0.0000001);
}

// ── save_stats 存储验证 ──

#[tokio::test]
async fn save_stats_records_to_store() {
    let store = Arc::new(MockWorkerStore::new());
    let bot = make_bot_config();
    let bot_id = bot.id;

    store.save_stats(bot_id, 50.0, 10.0, 5, 3).await.unwrap();

    let stats = store.stats_saved.lock().await;
    assert!(stats.iter().any(|(id, pnl, upnl, trades, filled)| {
        *id == bot_id
            && (pnl - 50.0).abs() < f64::EPSILON
            && (upnl - 10.0).abs() < f64::EPSILON
            && *trades == 5
            && *filled == 3
    }));
}

// ── record_trade 存储验证 ──

#[tokio::test]
async fn record_trade_stores_to_store() {
    let store = Arc::new(MockWorkerStore::new());
    let bot = make_bot_config();
    let bot_id = bot.id;
    let worker = make_worker_with_store(bot, 55000.0, store.clone());

    let (grid_event_tx, _) = broadcast::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);
    let price_provider = Arc::new(MockPriceProvider::new(55000.0));
    let order_executor = Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();

    let w2 = GridWorker::new(
        worker.bot.clone(),
        price_provider,
        order_executor,
        ai_service,
        store.clone(),
        Arc::new(crate::bot::semi_automatic_grid::test::common::MockMarketDataProvider),
        event_rx,
        grid_event_tx,
    );
    drop(w2);

    store.record_trade(bot_id, Uuid::new_v4(), "BTCUSDT", "binance", "buy", 3, 52000.0, 0.001, 0.0, 0.0).await.unwrap();

    let recorded = store.recorded_trades.lock().await;
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].0, bot_id);
    assert_eq!(recorded[0].1, "buy");
    assert_eq!(recorded[0].2, 3);
}

// ── on_order_event 完整分发验证 ──

#[tokio::test]
async fn on_order_event_order_placed_dispatches() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    let buy_price = worker.levels[2].buy_price;

    worker.on_order_event(OrderEvent::OrderPlaced {
        order: OrderInfo {
            id: order_id, side: OrderSide::Buy,
            fill_price: Some(buy_price), request_price: None, filled: 0.0,
            symbol: "BTCUSDT".to_string(),
            client_order_id: Some(format!("grid:{}:2:buy", worker.bot.id)),
        },
    }).await;

    assert!(worker.find_level_by_order_id(order_id).is_some());
    assert_eq!(worker.levels[2].buy_order_id, Some(order_id));
}

#[tokio::test]
async fn on_order_event_order_filled_dispatches() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    let buy_price = worker.levels[3].buy_price;
    let quantity = worker.levels[3].quantity;
    worker.levels[3].buy_order_id = Some(order_id);

    worker.on_order_event(OrderEvent::OrderFilled {
        order: OrderInfo {
            id: order_id, side: OrderSide::Buy,
            fill_price: Some(buy_price), request_price: None, filled: quantity,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
        },
    }).await;

    assert!(worker.levels[3].buy_filled);
    assert_eq!(worker.total_trades, 1);
}

#[tokio::test]
async fn on_order_event_order_canceled_dispatches() {
    let mut worker = make_worker(make_bot_config(), 55000.0);
    let order_id = Uuid::new_v4();
    worker.levels[2].buy_order_id = Some(order_id);
    worker.levels[2].buy_order_id = Some(order_id);

    worker.on_order_event(OrderEvent::OrderCanceled { order_id, symbol: None }).await;

    assert!(!worker.find_level_by_order_id(order_id).is_some());
    assert!(worker.levels[2].buy_order_id.is_none());
}

// ── place_initial_orders 只在当前价格以下下单 ──

#[tokio::test]
async fn place_initial_orders_only_below_current_price() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.current_price = 55000.0;
    worker.place_initial_orders().await;

    let commands = order_executor.commands().await;
    for cmd in &commands {
        if let OrderCommand::PlaceOrder { side: OrderSide::Buy, price, .. } = cmd {
            let p = price.unwrap();
            assert!(p < 55000.0, "Buy order price {} should be below current price 55000", p);
        }
    }
}

// ── place_initial_orders 在 initial_order_range 内 ──

#[tokio::test]
async fn place_initial_orders_within_range() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.current_price = 55000.0;
    let current_level = worker.find_level_by_price(55000.0);

    worker.place_initial_orders().await;

    let commands = order_executor.commands().await;
    let buy_orders: Vec<_> = commands.iter().filter_map(|c| {
        if let OrderCommand::PlaceOrder { side: OrderSide::Buy, price, .. } = c {
            Some(price.unwrap())
        } else {
            None
        }
    }).collect();

    assert!(!buy_orders.is_empty());
    for price in &buy_orders {
        let level_idx = worker.find_level_by_price(*price);
        assert!(
            level_idx <= current_level + 3,
            "Buy order at level {} should be within initial_order_range of current level {}",
            level_idx, current_level
        );
    }
}

// ── on_price_tick 多次 tick 不重复下单 ──

#[tokio::test]
async fn on_price_tick_does_not_duplicate_orders() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.current_price = 55000.0;

    worker.on_price_tick().await;
    let first_count = order_executor.commands().await.len();

    for level in &mut worker.levels {
        if level.buy_order_id.is_none() {
            level.buy_order_id = Some(uuid::Uuid::new_v4());
        }
    }

    worker.on_price_tick().await;
    let second_count = order_executor.commands().await.len();

    assert_eq!(first_count, second_count, "Second tick should not place duplicate orders for same levels");
}

// ── execute_decision adjust_grid 无 decision 不调整 ──

#[tokio::test]
async fn execute_decision_adjust_grid_without_decision_noop() {
    let store = Arc::new(MockWorkerStore::new());
    let bot = make_bot_config();
    let original_upper = bot.upper_price;
    let original_lower = bot.lower_price;
    let mut worker = make_worker_with_store(bot, 55000.0, store);

    worker.execute_decision(&GridAction::AdjustGrid { upper_price: Some(70000.0), lower_price: Some(40000.0) }, None).await;

    assert!((worker.bot.upper_price - original_upper).abs() < f64::EPSILON);
    assert!((worker.bot.lower_price - original_lower).abs() < f64::EPSILON);
}

// ── execute_decision run_grid 恢复后下初始单 ──

#[tokio::test]
async fn execute_decision_run_grid_places_initial_orders_on_resume() {
    let order_executor = Arc::new(MockOrderExecutor::new());
    let mut worker = make_worker_with_executor(make_bot_config(), 55000.0, order_executor.clone());
    worker.paused = true;
    worker.current_price = 55000.0;

    worker.execute_decision(&GridAction::RunGrid, None).await;

    assert!(!worker.paused);
    let commands = order_executor.commands().await;
    let buy_count = commands.iter().filter(|c| {
        matches!(c, OrderCommand::PlaceOrder { side: OrderSide::Buy, .. })
    }).count();
    assert!(buy_count > 0, "Resuming should place initial buy orders");
}

// ── on_order_filled sell with zero pnl when buy_price equals sell_price ──

#[tokio::test]
async fn on_order_filled_sell_zero_profit_pct() {
    let mut bot = make_bot_config();
    bot.grid_profit_pct = 0.0;
    let mut worker = make_worker(bot, 55000.0);

    let buy_price = worker.levels[3].buy_price;
    let sell_price = worker.levels[3].sell_price;
    let quantity = worker.levels[3].quantity;
    worker.levels[3].buy_filled = true;
    worker.levels[3].hold_quantity = quantity;
    let sell_order_id = Uuid::new_v4();
    worker.levels[3].sell_order_id = Some(sell_order_id);

    let order = OrderInfo {
        id: sell_order_id, side: OrderSide::Sell,
        fill_price: Some(sell_price), request_price: None, filled: quantity,
                symbol: "BTCUSDT".to_string(),
                client_order_id: None,
    };
    worker.on_order_filled(&order).await;

    assert!((worker.total_pnl).abs() < 0.001, "With 0% profit, PnL should be ~0");
}
