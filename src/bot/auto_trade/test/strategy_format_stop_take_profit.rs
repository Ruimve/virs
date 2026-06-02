/**
 * 测试 strategy::format_stop_take_profit 止损止盈信息格式化
 * - 两者都设置：显示止损价和止盈价
 * - 仅止损：只显示止损价
 * - 仅止盈：只显示止盈价
 * - 都未设置：返回"未设置"
 */
use crate::bot::auto_trade::strategy::format_stop_take_profit;

#[test]
fn both_set() {
    let info = format_stop_take_profit(95.0, 110.0);
    assert!(info.contains("95.00"), "should contain stop loss, got: {}", info);
    assert!(info.contains("110.00"), "should contain take profit, got: {}", info);
}

#[test]
fn only_stop_loss() {
    let info = format_stop_take_profit(95.0, 0.0);
    assert!(info.contains("95.00"), "should contain stop loss, got: {}", info);
    assert!(!info.contains("止盈"), "should not contain take profit, got: {}", info);
}

#[test]
fn only_take_profit() {
    let info = format_stop_take_profit(0.0, 110.0);
    assert!(info.contains("110.00"), "should contain take profit, got: {}", info);
    assert!(!info.contains("止损"), "should not contain stop loss, got: {}", info);
}

#[test]
fn neither_set() {
    let info = format_stop_take_profit(0.0, 0.0);
    assert_eq!(info, "未设置");
}

#[test]
fn negative_values_treated_as_unset() {
    let info = format_stop_take_profit(-1.0, -1.0);
    assert_eq!(info, "未设置");
}
