use super::common::*;
use super::super::tracker::PnlTracker;
use super::super::types::*;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================
// 初始化 (2 tests)
// ============================================================

#[test]
fn test_tracker_initial_state() {
    let tracker = PnlTracker::new(10000.0);
    assert_eq!(tracker.peak_equity(), 10000.0);
    assert_eq!(tracker.total_realized_pnl(), 0.0);
    assert_eq!(tracker.total_trades(), 0);
    assert_eq!(tracker.profit_trades(), 0);
    assert_eq!(tracker.total_cost(), 0.0);
}

#[test]
fn test_tracker_with_zero_initial_equity() {
    let tracker = PnlTracker::new(0.0);
    assert_eq!(tracker.peak_equity(), 0.0);
    assert_eq!(tracker.total_realized_pnl(), 0.0);
    assert_eq!(tracker.total_trades(), 0);
}

// ============================================================
// 未实现盈亏计算 (4 tests)
// ============================================================

#[test]
fn test_unrealized_pnl_long() {
    let mut tracker = PnlTracker::new(10000.0);

    let pos = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let mut prices = HashMap::new();
    prices.insert("BTCUSDT".to_string(), 110.0);

    let snap = tracker.update_unrealized(&[&pos], &prices);

    // Long: (110 - 100) * 1 = 10
    assert!((snap.unrealized_pnl - 10.0).abs() < 0.01);
}

#[test]
fn test_unrealized_pnl_short() {
    let mut tracker = PnlTracker::new(10000.0);

    let pos = make_position_side("BTCUSDT", PositionSide::Short, 1.0, 100.0, 10);
    let mut prices = HashMap::new();
    prices.insert("BTCUSDT".to_string(), 90.0);

    let snap = tracker.update_unrealized(&[&pos], &prices);

    // Short: (100 - 90) * 1 = 10
    assert!((snap.unrealized_pnl - 10.0).abs() < 0.01);
}

#[test]
fn test_unrealized_pnl_long_loss() {
    let mut tracker = PnlTracker::new(10000.0);

    let pos = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let mut prices = HashMap::new();
    prices.insert("BTCUSDT".to_string(), 90.0);

    let snap = tracker.update_unrealized(&[&pos], &prices);

    // Long: (90 - 100) * 1 = -10
    assert!((snap.unrealized_pnl - (-10.0)).abs() < 0.01);
}

#[test]
fn test_unrealized_pnl_multiple_positions() {
    let mut tracker = PnlTracker::new(10000.0);

    let btc_pos = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let eth_pos = make_position_side("ETHUSDT", PositionSide::Short, 1.0, 100.0, 10);

    let mut prices = HashMap::new();
    prices.insert("BTCUSDT".to_string(), 110.0);
    prices.insert("ETHUSDT".to_string(), 90.0);

    let snap = tracker.update_unrealized(&[&btc_pos, &eth_pos], &prices);

    // Long BTC: (110 - 100) * 1 = 10
    // Short ETH: (100 - 90) * 1 = 10
    // total = 20
    assert!((snap.unrealized_pnl - 20.0).abs() < 0.01);
}

// ============================================================
// 已实现盈亏 (3 tests)
// ============================================================

#[test]
fn test_record_trade_updates_realized() {
    let mut tracker = PnlTracker::new(10000.0);

    let mut trade = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    trade.pnl = 100.0;

    tracker.record_trade(&trade);

    assert_eq!(tracker.total_realized_pnl(), 100.0);
    assert_eq!(tracker.total_trades(), 1);
    assert_eq!(tracker.profit_trades(), 1);
}

#[test]
fn test_record_trade_negative_pnl() {
    let mut tracker = PnlTracker::new(10000.0);

    let mut trade = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    trade.pnl = -50.0;

    tracker.record_trade(&trade);

    assert_eq!(tracker.total_realized_pnl(), -50.0);
    assert_eq!(tracker.total_trades(), 1);
    assert_eq!(tracker.profit_trades(), 0);
}

#[test]
fn test_record_multiple_trades() {
    let mut tracker = PnlTracker::new(10000.0);

    let mut t1 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t1.pnl = 100.0;
    let mut t2 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t2.pnl = -50.0;
    let mut t3 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t3.pnl = 200.0;

    tracker.record_trade(&t1);
    tracker.record_trade(&t2);
    tracker.record_trade(&t3);

    assert_eq!(tracker.total_realized_pnl(), 250.0);
    assert_eq!(tracker.total_trades(), 3);
    assert_eq!(tracker.profit_trades(), 2);
}

// ============================================================
// Peak Equity 更新 (3 tests)
// ============================================================

#[test]
fn test_peak_equity_increases() {
    let mut tracker = PnlTracker::new(10000.0);

    let pos = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let mut prices = HashMap::new();
    prices.insert("BTCUSDT".to_string(), 110.0);

    // equity = 10000 + 0 + 10 = 10010 -> peak increases
    tracker.update_unrealized(&[&pos], &prices);

    assert_eq!(tracker.peak_equity(), 10010.0);
}

#[test]
fn test_peak_equity_not_decreases() {
    let mut tracker = PnlTracker::new(10000.0);

    // First: price goes up to 110 -> equity = 10000 + 10 = 10010 -> peak = 10010
    let pos = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let mut prices = HashMap::new();
    prices.insert("BTCUSDT".to_string(), 110.0);
    tracker.update_unrealized(&[&pos], &prices);
    assert_eq!(tracker.peak_equity(), 10010.0);

    // Then: price drops to 90 -> equity = 10000 - 10 = 9990 -> peak stays at 10010
    let pos2 = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let mut prices2 = HashMap::new();
    prices2.insert("BTCUSDT".to_string(), 90.0);
    tracker.update_unrealized(&[&pos2], &prices2);

    assert_eq!(tracker.peak_equity(), 10010.0);
}

#[test]
fn test_max_drawdown_calculation() {
    let mut tracker = PnlTracker::new(10000.0);

    // First: record a profit to push equity to 11000
    let mut t1 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t1.pnl = 1000.0;
    tracker.record_trade(&t1);
    // record_trade 现在也会更新 peak_equity
    assert_eq!(tracker.peak_equity(), 11000.0);

    // Then: record a loss to bring equity to 8000
    let mut t2 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t2.pnl = -2000.0;
    tracker.record_trade(&t2);
    let snap = tracker.snapshot(0.0, 0);

    // equity = 10000 + 1000 - 2000 = 9000
    // peak is 11000 (updated by record_trade)
    // drawdown = (11000 - 9000) / 11000 = 0.1818...
    let expected_dd = 2000.0 / 11000.0;
    assert!((snap.max_drawdown - expected_dd).abs() < 0.001);
}

// ============================================================
// 快照指标 (5 tests)
// ============================================================

#[test]
fn test_snapshot_win_rate() {
    let mut tracker = PnlTracker::new(10000.0);

    // 7 wins, 3 losses
    for _ in 0..7 {
        let mut t = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
        t.pnl = 10.0;
        tracker.record_trade(&t);
    }
    for _ in 0..3 {
        let mut t = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
        t.pnl = -10.0;
        tracker.record_trade(&t);
    }

    let snap = tracker.snapshot(0.0, 0);
    assert_eq!(snap.win_rate, Some(0.7));
}

#[test]
fn test_snapshot_win_rate_no_trades() {
    let tracker = PnlTracker::new(10000.0);

    let snap = tracker.snapshot(0.0, 0);
    assert!(snap.win_rate.is_none());
}

#[test]
fn test_snapshot_pnl_ratio() {
    let mut tracker = PnlTracker::new(10000.0);

    // total_profit = 1000, total_loss = 500
    let mut t1 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t1.pnl = 1000.0;
    tracker.record_trade(&t1);

    let mut t2 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t2.pnl = -500.0;
    tracker.record_trade(&t2);

    let snap = tracker.snapshot(0.0, 0);
    assert_eq!(snap.pnl_ratio, Some(2.0));
}

#[test]
fn test_snapshot_pnl_ratio_no_losses() {
    let mut tracker = PnlTracker::new(10000.0);

    let mut t1 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t1.pnl = 500.0;
    tracker.record_trade(&t1);

    let mut t2 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t2.pnl = 300.0;
    tracker.record_trade(&t2);

    let snap = tracker.snapshot(0.0, 0);
    // No losses -> pnl_ratio should be None
    assert!(snap.pnl_ratio.is_none());
}

#[test]
fn test_snapshot_avg_pnl_ratio() {
    let mut tracker = PnlTracker::new(10000.0);

    // 2 profit trades: 200 + 200 = 400 total, avg_profit = 200
    let mut t1 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t1.pnl = 200.0;
    tracker.record_trade(&t1);

    let mut t2 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t2.pnl = 200.0;
    tracker.record_trade(&t2);

    // 2 loss trades: 100 + 100 = 200 total, avg_loss = 100
    let mut t3 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t3.pnl = -100.0;
    tracker.record_trade(&t3);

    let mut t4 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t4.pnl = -100.0;
    tracker.record_trade(&t4);

    let snap = tracker.snapshot(0.0, 0);
    // avg_pnl_ratio = 200 / 100 = 2.0
    assert_eq!(snap.avg_pnl_ratio, Some(2.0));
}

// ============================================================
// 恢复 (2 tests)
// ============================================================

#[test]
fn test_restore_from_snapshot() {
    let mut tracker = PnlTracker::new(10000.0);

    tracker.restore_from_snapshot(15000.0, 3000.0, 10, 7, 5000.0, 2);

    assert_eq!(tracker.peak_equity(), 15000.0);
    assert_eq!(tracker.total_realized_pnl(), 3000.0);
    assert_eq!(tracker.total_trades(), 10);
    assert_eq!(tracker.profit_trades(), 7);
    assert_eq!(tracker.total_cost(), 5000.0);
    assert_eq!(tracker.consecutive_losses(), 2);
}

#[test]
fn test_restore_preserves_behavior() {
    let mut tracker = PnlTracker::new(10000.0);

    tracker.restore_from_snapshot(15000.0, 3000.0, 10, 7, 5000.0, 1);

    // Record a new trade after restore
    let mut trade = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    trade.pnl = 500.0;
    tracker.record_trade(&trade);

    // realized should be 3000 + 500 = 3500
    assert_eq!(tracker.total_realized_pnl(), 3500.0);
    // trades should be 10 + 1 = 11
    assert_eq!(tracker.total_trades(), 11);
    // profit_trades should be 7 + 1 = 8
    assert_eq!(tracker.profit_trades(), 8);
    // peak_equity should still be 15000 (snapshot doesn't update peak)
    assert_eq!(tracker.peak_equity(), 15000.0);
}

// ============================================================
// 开仓成本 (2 tests)
// ============================================================

#[test]
fn test_open_trade_increases_cost() {
    let mut tracker = PnlTracker::new(10000.0);

    let mut trade = make_trade(Uuid::nil(), Uuid::nil(), Side::Buy, 100.0, 1.0, TradeType::Open);
    trade.pnl = 0.0;
    trade.fee = 0.05;

    tracker.record_trade(&trade);

    // cost = price * amount + fee = 100 * 1 + 0.05 = 100.05
    assert!((tracker.total_cost() - 100.05).abs() < 0.001);
}

#[test]
fn test_close_trade_no_cost() {
    let mut tracker = PnlTracker::new(10000.0);

    // Record an open trade first
    let mut open_trade = make_trade(Uuid::nil(), Uuid::nil(), Side::Buy, 100.0, 1.0, TradeType::Open);
    open_trade.pnl = 0.0;
    open_trade.fee = 0.05;
    tracker.record_trade(&open_trade);
    assert!((tracker.total_cost() - 100.05).abs() < 0.001);

    // Record a close trade
    let mut close_trade = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    close_trade.pnl = 50.0;
    close_trade.fee = 0.05;
    tracker.record_trade(&close_trade);

    // cost should remain 100.05
    assert!((tracker.total_cost() - 100.05).abs() < 0.001);
}

// ============================================================
// 补充边界场景 (4 tests)
// ============================================================

#[test]
fn test_unrealized_pnl_position_side_both() {
    // PositionSide::Both 直接使用 pos.unrealized_pnl
    let mut tracker = PnlTracker::new(10000.0);
    let mut pos = make_position_side("BTCUSDT", PositionSide::Both, 1.0, 100.0, 10);
    pos.unrealized_pnl = 42.0;
    let mut prices = HashMap::new();
    prices.insert("BTCUSDT".to_string(), 200.0); // 价格变化不影响 Both
    let snap = tracker.update_unrealized(&[&pos], &prices);
    assert!((snap.unrealized_pnl - 42.0).abs() < 0.01);
}

#[test]
fn test_unrealized_pnl_missing_price() {
    // prices 中没有该 symbol，使用 pos.current_price
    // pos.current_price = 100, entry_price = 100 -> unrealized = 0
    let mut tracker = PnlTracker::new(10000.0);
    let pos = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let prices = HashMap::new(); // 空
    let snap = tracker.update_unrealized(&[&pos], &prices);
    assert!((snap.unrealized_pnl - 0.0).abs() < 0.01);
}

#[test]
fn test_snapshot_pnl_ratio_zero_loss() {
    // 全部亏损 -> pnl_ratio = Some(0.0)（total_profit_amount / total_loss_amount = 0/100）
    let mut tracker = PnlTracker::new(10000.0);
    let mut t = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t.pnl = -100.0;
    tracker.record_trade(&t);
    let snap = tracker.snapshot(0.0, 0);
    assert_eq!(snap.pnl_ratio, Some(0.0), "全部亏损时 pnl_ratio 应为 Some(0.0)");
}

#[test]
fn test_avg_pnl_ratio_only_profit() {
    // 只有盈利交易 -> avg_pnl_ratio=None
    let mut tracker = PnlTracker::new(10000.0);
    let mut t = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t.pnl = 100.0;
    tracker.record_trade(&t);
    let snap = tracker.snapshot(0.0, 0);
    assert!(snap.avg_pnl_ratio.is_none(), "只有盈利时 avg_pnl_ratio 应为 None");
}

// ============================================================
// 更多边界场景
// ============================================================

#[test]
fn test_equity_method() {
    let mut tracker = PnlTracker::new(10000.0);
    assert!((tracker.equity() - 10000.0).abs() < 0.01);

    let mut t = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t.pnl = 500.0;
    tracker.record_trade(&t);
    assert!((tracker.equity() - 10500.0).abs() < 0.01);

    let mut t2 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t2.pnl = -200.0;
    tracker.record_trade(&t2);
    assert!((tracker.equity() - 10300.0).abs() < 0.01);
}

#[test]
fn test_unrealized_pnl_empty_positions() {
    let mut tracker = PnlTracker::new(10000.0);
    let prices = HashMap::new();
    let snap = tracker.update_unrealized(&[], &prices);
    assert!((snap.unrealized_pnl - 0.0).abs() < 0.01);
    assert!((snap.equity - 10000.0).abs() < 0.01);
    assert_eq!(snap.open_positions_count, 0);
}

#[test]
fn test_unrealized_pnl_short_loss() {
    let mut tracker = PnlTracker::new(10000.0);
    let pos = make_position_side("BTCUSDT", PositionSide::Short, 1.0, 100.0, 10);
    let mut prices = HashMap::new();
    prices.insert("BTCUSDT".to_string(), 110.0);
    let snap = tracker.update_unrealized(&[&pos], &prices);
    assert!((snap.unrealized_pnl - (-10.0)).abs() < 0.01);
}

#[test]
fn test_record_trade_zero_pnl() {
    let mut tracker = PnlTracker::new(10000.0);
    let mut t = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t.pnl = 0.0;
    tracker.record_trade(&t);
    assert_eq!(tracker.total_realized_pnl(), 0.0);
    assert_eq!(tracker.total_trades(), 1);
    assert_eq!(tracker.profit_trades(), 0, "pnl=0 不计入盈利交易");
}

#[test]
fn test_record_trade_zero_fee() {
    let mut tracker = PnlTracker::new(10000.0);
    let mut t = make_trade(Uuid::nil(), Uuid::nil(), Side::Buy, 100.0, 1.0, TradeType::Open);
    t.pnl = 0.0;
    t.fee = 0.0;
    tracker.record_trade(&t);
    assert!((tracker.total_cost() - 100.0).abs() < 0.001);
}

#[test]
fn test_snapshot_no_drawdown() {
    let tracker = PnlTracker::new(10000.0);
    let snap = tracker.snapshot(0.0, 0);
    assert!((snap.max_drawdown - 0.0).abs() < 0.001, "无交易无持仓时回撤应为 0");
}

#[test]
fn test_snapshot_equity_above_peak() {
    let mut tracker = PnlTracker::new(10000.0);

    let pos = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let mut prices = HashMap::new();
    prices.insert("BTCUSDT".to_string(), 200.0);
    let snap = tracker.update_unrealized(&[&pos], &prices);
    assert!((snap.equity - 10100.0).abs() < 0.01);
    assert!((snap.max_drawdown - 0.0).abs() < 0.001, "权益等于峰值时回撤应为 0");
}

#[test]
fn test_update_unrealized_then_restore_then_update() {
    let mut tracker = PnlTracker::new(10000.0);

    let pos = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let mut prices = HashMap::new();
    prices.insert("BTCUSDT".to_string(), 110.0);
    tracker.update_unrealized(&[&pos], &prices);
    assert_eq!(tracker.peak_equity(), 10010.0);

    tracker.restore_from_snapshot(20000.0, 5000.0, 20, 15, 8000.0, 3);
    assert_eq!(tracker.peak_equity(), 20000.0);

    let pos2 = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let mut prices2 = HashMap::new();
    prices2.insert("BTCUSDT".to_string(), 120.0);
    let snap = tracker.update_unrealized(&[&pos2], &prices2);
    assert!((snap.equity - 15020.0).abs() < 0.01, "equity = initial(10000) + realized(5000) + unrealized(20)");
}

#[test]
fn test_peak_equity_multiple_updates() {
    let mut tracker = PnlTracker::new(10000.0);

    let pos = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);

    let mut prices1 = HashMap::new();
    prices1.insert("BTCUSDT".to_string(), 120.0);
    tracker.update_unrealized(&[&pos], &prices1);
    assert_eq!(tracker.peak_equity(), 10020.0);

    let mut prices2 = HashMap::new();
    prices2.insert("BTCUSDT".to_string(), 130.0);
    tracker.update_unrealized(&[&pos], &prices2);
    assert_eq!(tracker.peak_equity(), 10030.0);

    let mut prices3 = HashMap::new();
    prices3.insert("BTCUSDT".to_string(), 110.0);
    tracker.update_unrealized(&[&pos], &prices3);
    assert_eq!(tracker.peak_equity(), 10030.0, "价格下跌后 peak 不应降低");
}

#[test]
fn test_snapshot_all_loss_trades() {
    let mut tracker = PnlTracker::new(10000.0);
    for _ in 0..5 {
        let mut t = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
        t.pnl = -50.0;
        tracker.record_trade(&t);
    }
    let snap = tracker.snapshot(0.0, 0);
    assert_eq!(snap.win_rate, Some(0.0), "全部亏损时胜率应为 0");
    assert_eq!(snap.pnl_ratio, Some(0.0), "全部亏损时 pnl_ratio 应为 0");
}

#[test]
fn test_snapshot_drawdown_after_loss() {
    let mut tracker = PnlTracker::new(10000.0);

    let pos = make_position_side("BTCUSDT", PositionSide::Long, 1.0, 100.0, 10);
    let mut prices_up = HashMap::new();
    prices_up.insert("BTCUSDT".to_string(), 200.0);
    tracker.update_unrealized(&[&pos], &prices_up);
    assert_eq!(tracker.peak_equity(), 10100.0);

    let mut prices_down = HashMap::new();
    prices_down.insert("BTCUSDT".to_string(), 50.0);
    let snap = tracker.update_unrealized(&[&pos], &prices_down);
    let expected_dd = (10100.0 - 9950.0) / 10100.0;
    assert!((snap.max_drawdown - expected_dd).abs() < 0.001);
}

#[test]
fn test_record_trade_profit_and_loss_mixed() {
    let mut tracker = PnlTracker::new(10000.0);

    let mut t1 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t1.pnl = 300.0;
    tracker.record_trade(&t1);

    let mut t2 = make_trade(Uuid::nil(), Uuid::nil(), Side::Sell, 100.0, 1.0, TradeType::Close);
    t2.pnl = -100.0;
    tracker.record_trade(&t2);

    assert_eq!(tracker.total_realized_pnl(), 200.0);
    assert_eq!(tracker.total_trades(), 2);
    assert_eq!(tracker.profit_trades(), 1);

    let snap = tracker.snapshot(0.0, 0);
    assert_eq!(snap.pnl_ratio, Some(3.0));
    assert_eq!(snap.avg_pnl_ratio, Some(3.0));
}
