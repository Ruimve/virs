/**
 * 测试 strategy::compute_stop_loss 止损价计算
 * - long 方向：entry_price - 1.5 * ATR
 * - short 方向：entry_price + 1.5 * ATR
 * - 未知方向：fallback entry_price * 0.97
 * - ATR <= 0 或 entry_price <= 0：fallback entry_price * 0.97
 */
use crate::bot::auto_trade::strategy::compute_stop_loss;

#[test]
fn long_stop_loss_normal() {
    let sl = compute_stop_loss(100.0, "long", 2.0);
    let expected = 100.0 - 1.5 * 2.0;
    assert!((sl - expected).abs() < 0.001, "expected {}, got {}", expected, sl);
}

#[test]
fn short_stop_loss_normal() {
    let sl = compute_stop_loss(100.0, "short", 2.0);
    let expected = 100.0 + 1.5 * 2.0;
    assert!((sl - expected).abs() < 0.001, "expected {}, got {}", expected, sl);
}

#[test]
fn unknown_side_fallback() {
    let sl = compute_stop_loss(100.0, "other", 2.0);
    let expected = 100.0 * 0.97;
    assert!((sl - expected).abs() < 0.001, "expected {}, got {}", expected, sl);
}

#[test]
fn zero_atr_fallback() {
    let sl = compute_stop_loss(100.0, "long", 0.0);
    let expected = 100.0 * 0.97;
    assert!((sl - expected).abs() < 0.001, "expected {}, got {}", expected, sl);
}

#[test]
fn negative_atr_fallback() {
    let sl = compute_stop_loss(100.0, "long", -1.0);
    let expected = 100.0 * 0.97;
    assert!((sl - expected).abs() < 0.001, "expected {}, got {}", expected, sl);
}

#[test]
fn zero_entry_price_fallback() {
    let sl = compute_stop_loss(0.0, "long", 2.0);
    assert_eq!(sl, 0.0, "expected 0.0 for zero entry_price");
}

#[test]
fn long_stop_loss_with_large_atr() {
    let sl = compute_stop_loss(100.0, "long", 100.0);
    let expected = 100.0 - 1.5 * 100.0;
    assert!((sl - expected).abs() < 0.001, "expected {}, got {}", expected, sl);
    assert!(sl < 0.0, "stop loss can be negative with large ATR");
}
