/**
 * 测试 strategy::compute_trailing_stop 移动止损计算
 * - long: 盈利>=2ATR → current_price - 1*ATR; 盈利>=1ATR → entry_price; 否则不变
 * - short: 盈利>=2ATR → current_price + 1*ATR; 盈利>=1ATR → entry_price; 否则不变
 * - 只向有利方向移动（long 新止损>旧止损, short 新止损<旧止损）
 * - ATR<=0 或 entry_price<=0 → 返回 current_stop
 */
use crate::bot::auto_trade::strategy::compute_trailing_stop;

#[test]
fn long_profit_2atr_locks_1atr() {
    let entry = 100.0;
    let current = 110.0;
    let atr = 5.0;
    let current_stop = 92.5;
    let new_stop = compute_trailing_stop(entry, current, "long", atr, current_stop);
    let expected = current - 1.0 * atr;
    assert!((new_stop - expected).abs() < 0.001, "expected {}, got {}", expected, new_stop);
    assert!(new_stop > current_stop, "trailing stop should move up for long");
}

#[test]
fn long_profit_1atr_moves_to_breakeven() {
    let entry = 100.0;
    let current = 105.0;
    let atr = 5.0;
    let current_stop = 92.5;
    let new_stop = compute_trailing_stop(entry, current, "long", atr, current_stop);
    assert!((new_stop - entry).abs() < 0.001, "expected breakeven {}, got {}", entry, new_stop);
}

#[test]
fn long_no_profit_keeps_stop() {
    let entry = 100.0;
    let current = 103.0;
    let atr = 5.0;
    let current_stop = 92.5;
    let new_stop = compute_trailing_stop(entry, current, "long", atr, current_stop);
    assert!((new_stop - current_stop).abs() < 0.001, "expected {}, got {}", current_stop, new_stop);
}

#[test]
fn long_new_stop_lower_than_current_keeps_current() {
    let entry = 100.0;
    let current = 105.0;
    let atr = 5.0;
    let current_stop = 101.0;
    let new_stop = compute_trailing_stop(entry, current, "long", atr, current_stop);
    assert!((new_stop - current_stop).abs() < 0.001, "should not lower the stop for long");
}

#[test]
fn short_profit_2atr_locks_1atr() {
    let entry = 100.0;
    let current = 90.0;
    let atr = 5.0;
    let current_stop = 107.5;
    let new_stop = compute_trailing_stop(entry, current, "short", atr, current_stop);
    let expected = current + 1.0 * atr;
    assert!((new_stop - expected).abs() < 0.001, "expected {}, got {}", expected, new_stop);
    assert!(new_stop < current_stop, "trailing stop should move down for short");
}

#[test]
fn short_profit_1atr_moves_to_breakeven() {
    let entry = 100.0;
    let current = 95.0;
    let atr = 5.0;
    let current_stop = 107.5;
    let new_stop = compute_trailing_stop(entry, current, "short", atr, current_stop);
    assert!((new_stop - entry).abs() < 0.001, "expected breakeven {}, got {}", entry, new_stop);
}

#[test]
fn short_no_profit_keeps_stop() {
    let entry = 100.0;
    let current = 97.0;
    let atr = 5.0;
    let current_stop = 107.5;
    let new_stop = compute_trailing_stop(entry, current, "short", atr, current_stop);
    assert!((new_stop - current_stop).abs() < 0.001, "expected {}, got {}", current_stop, new_stop);
}

#[test]
fn short_new_stop_higher_than_current_keeps_current() {
    let entry = 100.0;
    let current = 95.0;
    let atr = 5.0;
    let current_stop = 99.0;
    let new_stop = compute_trailing_stop(entry, current, "short", atr, current_stop);
    assert!((new_stop - current_stop).abs() < 0.001, "should not raise the stop for short");
}

#[test]
fn short_zero_current_stop_accepts_new() {
    let entry = 100.0;
    let current = 90.0;
    let atr = 5.0;
    let current_stop = 0.0;
    let new_stop = compute_trailing_stop(entry, current, "short", atr, current_stop);
    let expected = current + 1.0 * atr;
    assert!((new_stop - expected).abs() < 0.001, "expected {}, got {}", expected, new_stop);
}

#[test]
fn zero_atr_returns_current_stop() {
    let new_stop = compute_trailing_stop(100.0, 110.0, "long", 0.0, 92.5);
    assert!((new_stop - 92.5).abs() < 0.001);
}

#[test]
fn zero_entry_price_returns_current_stop() {
    let new_stop = compute_trailing_stop(0.0, 110.0, "long", 5.0, 92.5);
    assert!((new_stop - 92.5).abs() < 0.001);
}

#[test]
fn unknown_side_returns_current_stop() {
    let new_stop = compute_trailing_stop(100.0, 110.0, "other", 5.0, 92.5);
    assert!((new_stop - 92.5).abs() < 0.001);
}
