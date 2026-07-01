//! Unit tests for grid/utils/prompt.rs

use crate::grid::utils::prompt::format_bars_outside;

#[test]
fn p1_1_format_bars_positive() {
    let result = format_bars_outside(3);
    assert_eq!(result, "向上3根");
}

#[test]
fn p1_2_format_bars_negative() {
    let result = format_bars_outside(-2);
    assert_eq!(result, "向下2根");
}

#[test]
fn p1_3_format_bars_zero() {
    let result = format_bars_outside(0);
    assert_eq!(result, "无");
}
