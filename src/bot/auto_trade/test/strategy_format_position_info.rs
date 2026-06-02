/**
 * 测试 strategy::format_position_info 仓位信息格式化
 * - long 仓位盈利：显示方向、入场价、持仓量、当前价、正未实现盈亏
 * - long 仓位亏损：显示负未实现盈亏
 * - short 仓位盈利/亏损：盈亏计算方向相反
 * - 无仓位（none/空/None）：返回"无仓位"
 * - entry_price=0 时盈亏百分比为0（格式化后含正号）
 */
use crate::bot::auto_trade::strategy::format_position_info;

#[test]
fn long_position_shows_side() {
    let info = format_position_info(Some("long"), 100.0, 0.1, 110.0);
    assert!(info.contains("long"), "should contain side 'long', got: {}", info);
}

#[test]
fn long_position_shows_entry_price() {
    let info = format_position_info(Some("long"), 100.0, 0.1, 110.0);
    assert!(info.contains("100.00"), "should contain entry price, got: {}", info);
}

#[test]
fn long_position_shows_current_price() {
    let info = format_position_info(Some("long"), 100.0, 0.1, 110.0);
    assert!(info.contains("110.00"), "should contain current price, got: {}", info);
}

#[test]
fn long_position_unrealized_pnl_positive() {
    let info = format_position_info(Some("long"), 100.0, 1.0, 110.0);
    assert!(info.contains("+10.00%"), "should show +10% pnl, got: {}", info);
}

#[test]
fn long_position_unrealized_pnl_negative() {
    let info = format_position_info(Some("long"), 100.0, 1.0, 90.0);
    assert!(info.contains("-10.00%"), "should show -10% pnl, got: {}", info);
}

#[test]
fn short_position_unrealized_pnl_positive() {
    let info = format_position_info(Some("short"), 100.0, 1.0, 90.0);
    assert!(info.contains("+10.00%"), "should show +10% pnl for short, got: {}", info);
}

#[test]
fn short_position_unrealized_pnl_negative() {
    let info = format_position_info(Some("short"), 100.0, 1.0, 110.0);
    assert!(info.contains("-10.00%"), "should show -10% pnl for short, got: {}", info);
}

#[test]
fn no_position_none_string() {
    assert_eq!(format_position_info(Some("none"), 0.0, 0.0, 0.0), "无仓位");
}

#[test]
fn no_position_empty_string() {
    assert_eq!(format_position_info(Some(""), 0.0, 0.0, 0.0), "无仓位");
}

#[test]
fn no_position_none_option() {
    assert_eq!(format_position_info(None, 0.0, 0.0, 0.0), "无仓位");
}

#[test]
fn zero_entry_price_pnl_pct_is_zero_with_sign() {
    let info = format_position_info(Some("long"), 0.0, 1.0, 110.0);
    assert!(info.contains("+0.00%"), "pnl pct should be +0.00% for zero entry, got: {}", info);
}
