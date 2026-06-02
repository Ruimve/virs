/**
 * 测试 strategy::compute_take_profit 止盈价计算
 * - long 方向：entry_price + 3.0 * ATR
 * - short 方向：entry_price - 3.0 * ATR
 * - 未知方向：fallback entry_price * 1.06
 * - ATR <= 0 或 entry_price <= 0：fallback entry_price * 1.06
 */
use crate::bot::auto_trade::strategy::compute_take_profit;

#[test]
fn long_take_profit_normal() {
    let tp = compute_take_profit(100.0, "long", 2.0);
    let expected = 100.0 + 3.0 * 2.0;
    assert!((tp - expected).abs() < 0.001, "expected {}, got {}", expected, tp);
}

#[test]
fn short_take_profit_normal() {
    let tp = compute_take_profit(100.0, "short", 2.0);
    let expected = 100.0 - 3.0 * 2.0;
    assert!((tp - expected).abs() < 0.001, "expected {}, got {}", expected, tp);
}

#[test]
fn unknown_side_fallback() {
    let tp = compute_take_profit(100.0, "other", 2.0);
    let expected = 100.0 * 1.06;
    assert!((tp - expected).abs() < 0.001, "expected {}, got {}", expected, tp);
}

#[test]
fn zero_atr_fallback() {
    let tp = compute_take_profit(100.0, "long", 0.0);
    let expected = 100.0 * 1.06;
    assert!((tp - expected).abs() < 0.001, "expected {}, got {}", expected, tp);
}

#[test]
fn negative_atr_fallback() {
    let tp = compute_take_profit(100.0, "short", -1.0);
    let expected = 100.0 * 1.06;
    assert!((tp - expected).abs() < 0.001, "expected {}, got {}", expected, tp);
}

#[test]
fn zero_entry_price_fallback() {
    let tp = compute_take_profit(0.0, "long", 2.0);
    assert_eq!(tp, 0.0, "expected 0.0 for zero entry_price");
}

#[test]
fn short_take_profit_with_large_atr() {
    let tp = compute_take_profit(100.0, "short", 50.0);
    let expected = 100.0 - 3.0 * 50.0;
    assert!((tp - expected).abs() < 0.001, "expected {}, got {}", expected, tp);
    assert!(tp < 0.0, "take profit can be negative with large ATR");
}
