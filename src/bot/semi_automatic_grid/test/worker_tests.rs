use super::common::*;
use crate::bot::semi_automatic_grid::ai::GridAction;
use crate::bot::semi_automatic_grid::ports::*;
use crate::bot::semi_automatic_grid::worker::GridWorker;
use tokio::sync::broadcast;
use uuid::Uuid;

// ── calculate_levels tests ──

#[test]
fn calculate_levels_basic() {
    let bot = make_bot_config();
    let levels = GridWorker::calculate_levels(&bot);
    assert_eq!(levels.len(), 10);
    let grid_spacing = (60000.0 - 50000.0) / 10.0;
    let profit_factor = 1.0 + 0.5 / 100.0;
    for (i, level) in levels.iter().enumerate() {
        let expected_buy = 50000.0 + grid_spacing * (i as f64 + 0.5);
        let expected_sell = expected_buy * profit_factor;
        let expected_qty = 100.0 / expected_buy;
        assert_eq!(level.level, i as i32);
        assert!((level.buy_price - expected_buy).abs() < 0.01);
        assert!((level.sell_price - expected_sell).abs() < 0.01);
        assert!((level.quantity - expected_qty).abs() < 0.0000001);
        assert!(level.buy_order_id.is_none());
        assert!(level.sell_order_id.is_none());
        assert!(!level.buy_filled);
        assert!(!level.sell_filled);
        assert!((level.hold_quantity - 0.0).abs() < f64::EPSILON);
    }
}

#[test]
fn calculate_levels_single() {
    let mut bot = make_bot_config();
    bot.grid_count = 1;
    let levels = GridWorker::calculate_levels(&bot);
    assert_eq!(levels.len(), 1);
    let grid_spacing = (60000.0 - 50000.0) / 1.0;
    let expected_buy = 50000.0 + grid_spacing * 0.5;
    assert!((levels[0].buy_price - expected_buy).abs() < 0.01);
}

#[test]
fn calculate_levels_zero_count() {
    let mut bot = make_bot_config();
    bot.grid_count = 0;
    assert!(GridWorker::calculate_levels(&bot).is_empty());
}

#[test]
fn calculate_levels_zero_prices_upper() {
    let mut bot = make_bot_config();
    bot.upper_price = 0.0;
    assert!(GridWorker::calculate_levels(&bot).is_empty());
}

#[test]
fn calculate_levels_zero_prices_lower() {
    let mut bot = make_bot_config();
    bot.lower_price = 0.0;
    assert!(GridWorker::calculate_levels(&bot).is_empty());
}

#[test]
fn calculate_levels_profit_pct() {
    let mut bot = make_bot_config();
    bot.grid_count = 1;
    bot.upper_price = 200.0;
    bot.lower_price = 100.0;
    bot.grid_profit_pct = 2.0;
    let levels = GridWorker::calculate_levels(&bot);
    assert_eq!(levels.len(), 1);
    let buy_price = levels[0].buy_price;
    let sell_price = levels[0].sell_price;
    let expected_sell = buy_price * (1.0 + 2.0 / 100.0);
    assert!((sell_price - expected_sell).abs() < 0.01);
}

// ── find_level_by_price tests ──

#[test]
fn find_level_by_price_exact() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let target = worker.levels[4].buy_price;
    assert_eq!(worker.find_level_by_price(target), 4);
}

#[test]
fn find_level_by_price_between() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let between = 55200.0;
    assert_eq!(worker.find_level_by_price(between), 5);
}

// ── simple_rule_decision tests ──

#[test]
fn simple_rule_decision_above_upper() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 62000.0);
    assert!(matches!(worker.simple_rule_decision(), GridAction::PauseGrid));
}

#[test]
fn simple_rule_decision_below_lower() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 48000.0);
    assert!(matches!(worker.simple_rule_decision(), GridAction::PauseGrid));
}

#[test]
fn simple_rule_decision_in_range() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    worker.current_price = 55000.0;
    worker.paused = false;
    assert!(matches!(worker.simple_rule_decision(), GridAction::Hold));
}

#[test]
fn simple_rule_decision_paused_in_range() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    worker.current_price = 55000.0;
    worker.paused = true;
    assert!(matches!(worker.simple_rule_decision(), GridAction::RunGrid));
}

// ── on_order_placed tests ──

#[tokio::test]
async fn on_order_placed_via_map() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let order_id = Uuid::new_v4();
    worker.order_level_map.insert(order_id, (3, "buy".to_string()));
    let order = GridOrderInfo {
        id: order_id, side: GridSide::Buy,
        fill_price: Some(worker.levels[3].buy_price),
        request_price: None, filled: 0.0,
    };
    worker.on_order_placed(&order).await;
    assert_eq!(worker.order_level_map.len(), 1);
}

#[tokio::test]
async fn on_order_placed_via_price() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let order_id = Uuid::new_v4();
    let buy_price = worker.levels[2].buy_price;
    let order = GridOrderInfo {
        id: order_id, side: GridSide::Buy,
        fill_price: Some(buy_price), request_price: None, filled: 0.0,
    };
    worker.on_order_placed(&order).await;
    assert!(worker.order_level_map.contains_key(&order_id));
    let (idx, side) = worker.order_level_map.get(&order_id).unwrap();
    assert_eq!(*idx, 2);
    assert_eq!(side, "buy");
    assert_eq!(worker.levels[2].buy_order_id, Some(order_id));
}

#[tokio::test]
async fn on_order_placed_no_match() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let order_id = Uuid::new_v4();
    let order = GridOrderInfo {
        id: order_id, side: GridSide::Buy,
        fill_price: Some(99999.0), request_price: None, filled: 0.0,
    };
    worker.on_order_placed(&order).await;
    assert!(!worker.order_level_map.contains_key(&order_id));
}

// ── on_order_filled tests ──

#[tokio::test]
async fn on_order_filled_buy() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let order_id = Uuid::new_v4();
    let buy_price = worker.levels[3].buy_price;
    let quantity = worker.levels[3].quantity;
    worker.order_level_map.insert(order_id, (3, "buy".to_string()));
    let order = GridOrderInfo {
        id: order_id, side: GridSide::Buy,
        fill_price: Some(buy_price), request_price: None, filled: quantity,
    };
    worker.on_order_filled(&order).await;
    assert!(worker.levels[3].buy_filled);
    assert!(worker.levels[3].buy_order_id.is_none());
    assert!((worker.levels[3].hold_quantity - quantity).abs() < f64::EPSILON);
    assert_eq!(worker.total_trades, 1);
    assert_eq!(worker.grid_filled_count, 1);
    assert!(!worker.order_level_map.contains_key(&order_id));
}

#[tokio::test]
async fn on_order_filled_sell() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let buy_price = worker.levels[3].buy_price;
    let sell_price = worker.levels[3].sell_price;
    let quantity = worker.levels[3].quantity;
    worker.levels[3].buy_filled = true;
    worker.levels[3].hold_quantity = quantity;
    let sell_order_id = Uuid::new_v4();
    worker.order_level_map.insert(sell_order_id, (3, "sell".to_string()));
    worker.levels[3].sell_order_id = Some(sell_order_id);
    let order = GridOrderInfo {
        id: sell_order_id, side: GridSide::Sell,
        fill_price: Some(sell_price), request_price: None, filled: quantity,
    };
    worker.on_order_filled(&order).await;
    assert!(worker.levels[3].sell_filled);
    assert!(worker.levels[3].sell_order_id.is_none());
    assert!((worker.levels[3].hold_quantity - 0.0).abs() < f64::EPSILON);
    assert!(worker.total_pnl > 0.0);
    assert_eq!(worker.total_trades, 1);
    assert!(!worker.order_level_map.contains_key(&sell_order_id));
}

#[tokio::test]
async fn on_order_filled_no_match() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let order_id = Uuid::new_v4();
    let order = GridOrderInfo {
        id: order_id, side: GridSide::Buy,
        fill_price: Some(50000.0), request_price: None, filled: 0.001,
    };
    worker.on_order_filled(&order).await;
    assert_eq!(worker.total_trades, 0);
    assert_eq!(worker.grid_filled_count, 0);
}

// ── clear_order_id tests ──

#[tokio::test]
async fn clear_order_id() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let order_id = Uuid::new_v4();
    worker.order_level_map.insert(order_id, (2, "buy".to_string()));
    worker.levels[2].buy_order_id = Some(order_id);
    worker.clear_order_id(order_id);
    assert!(!worker.order_level_map.contains_key(&order_id));
    assert!(worker.levels[2].buy_order_id.is_none());
}

// ── on_order_canceled tests ──

#[tokio::test]
async fn on_order_canceled() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let order_id = Uuid::new_v4();
    worker.order_level_map.insert(order_id, (1, "buy".to_string()));
    worker.levels[1].buy_order_id = Some(order_id);
    worker.on_order_canceled(order_id).await;
    assert!(!worker.order_level_map.contains_key(&order_id));
    assert!(worker.levels[1].buy_order_id.is_none());
}

// ── on_risk_alert tests ──

#[tokio::test]
async fn on_risk_alert_closeall() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    assert!(!worker.paused);
    worker.on_order_event(GridOrderEvent::RiskAlert {
        level: "CloseAll".to_string(),
        message: "Risk!".to_string(),
    }).await;
    assert!(worker.paused);
}

#[tokio::test]
async fn on_risk_alert_other_level() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    worker.on_order_event(GridOrderEvent::RiskAlert {
        level: "Info".to_string(),
        message: "Just info".to_string(),
    }).await;
    assert!(!worker.paused);
}

// ── on_liquidation_warning tests ──

#[tokio::test]
async fn on_liquidation_warning() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    assert!(!worker.paused);
    worker.on_order_event(GridOrderEvent::LiquidationWarning {
        symbol: "BTCUSDT".to_string(),
        liquidation_price: 45000.0,
        current_price: 46000.0,
    }).await;
    assert!(worker.paused);
}

// ── adjust_grid tests ──

#[tokio::test]
async fn adjust_grid_with_changes() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let old_upper = worker.bot.upper_price;
    let old_lower = worker.bot.lower_price;
    worker.adjust_grid(Some(65000.0), Some(48000.0)).await;
    assert!((worker.bot.upper_price - 65000.0).abs() < f64::EPSILON);
    assert!((worker.bot.lower_price - 48000.0).abs() < f64::EPSILON);
    assert_ne!(worker.bot.upper_price, old_upper);
    assert_ne!(worker.bot.lower_price, old_lower);
    assert_eq!(worker.levels.len(), 10);
}

#[tokio::test]
async fn adjust_grid_no_changes() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let old_levels_count = worker.levels.len();
    worker.adjust_grid(Some(60000.0), Some(50000.0)).await;
    assert!((worker.bot.upper_price - 60000.0).abs() < f64::EPSILON);
    assert!((worker.bot.lower_price - 50000.0).abs() < f64::EPSILON);
    assert_eq!(worker.levels.len(), old_levels_count);
}

// ── fetch_current_price tests ──

#[tokio::test]
async fn fetch_current_price_valid() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    worker.current_price = 10000.0;
    let price = worker.fetch_current_price().await;
    assert!((price - 55000.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn fetch_current_price_invalid() {
    let bot = make_bot_config();
    let (event_tx, event_rx) = broadcast::channel(16);
    let (grid_event_tx, _) = broadcast::channel(16);
    let price_provider = std::sync::Arc::new(MockPriceProvider::new(0.0));
    let order_executor = std::sync::Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();
    let store = std::sync::Arc::new(MockWorkerStore::new());
    let mut worker = GridWorker::new(bot, price_provider, order_executor, ai_service, store, event_rx, grid_event_tx);
    worker.current_price = 12345.0;
    let price = worker.fetch_current_price().await;
    assert!((price - 12345.0).abs() < f64::EPSILON);
}

// ── pause_with_cancel tests ──

#[tokio::test]
async fn pause_with_cancel_already_paused() {
    let bot = make_bot_config();
    let (event_tx, event_rx) = broadcast::channel(16);
    let (grid_event_tx, _) = broadcast::channel(16);
    let price_provider = std::sync::Arc::new(MockPriceProvider::new(55000.0));
    let order_executor = std::sync::Arc::new(MockOrderExecutor::new());
    let ai_service = make_mock_ai_service();
    let store = std::sync::Arc::new(MockWorkerStore::new());
    let mut worker = GridWorker::new(bot, price_provider, order_executor.clone(), ai_service, store, event_rx, grid_event_tx);
    worker.paused = true;
    worker.pause_with_cancel("test").await;
    assert!(worker.paused);
    let commands = order_executor.commands().await;
    assert!(commands.is_empty());
}

// ── execute_decision tests ──

#[tokio::test]
async fn execute_decision_reduce_position() {
    let bot = make_bot_config();
    let mut worker = make_worker(bot, 55000.0);
    let original_qty = worker.bot.quantity_per_grid;
    assert!((original_qty - 100.0).abs() < f64::EPSILON);
    worker.execute_decision(&GridAction::ReducePosition, None).await;
    assert!((worker.bot.quantity_per_grid - 50.0).abs() < f64::EPSILON);
}

// ── record_trade tests ──

#[test]
fn record_trade_pnl_pct_calculation() {
    let price = 55000.0;
    let quantity = 0.001;
    let pnl = 5.5;
    let pnl_pct = if price > 0.0 { pnl / (price * quantity) * 100.0 } else { 0.0 };
    assert!((pnl_pct - 10.0_f64).abs() < 0.001);
}
