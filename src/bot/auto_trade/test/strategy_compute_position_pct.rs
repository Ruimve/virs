/**
 * 测试 strategy::compute_position_pct 仓位百分比计算
 * - ADX>=25: base=80; ADX>=20: base=60; ADX<20: base=40
 * - 连续亏损>=2: base*0.5
 * - |funding_rate|>0.001: after_loss*0.5
 * - 结果 clamp(10.0, 100.0)
 */
use crate::bot::auto_trade::strategy::compute_position_pct;

#[test]
fn high_adx_no_losses_no_funding() {
    let pct = compute_position_pct(30.0, 0, 0.0);
    assert!((pct - 80.0).abs() < 0.001, "expected 80.0, got {}", pct);
}

#[test]
fn medium_adx_no_losses_no_funding() {
    let pct = compute_position_pct(22.0, 0, 0.0);
    assert!((pct - 60.0).abs() < 0.001, "expected 60.0, got {}", pct);
}

#[test]
fn low_adx_no_losses_no_funding() {
    let pct = compute_position_pct(15.0, 0, 0.0);
    assert!((pct - 40.0).abs() < 0.001, "expected 40.0, got {}", pct);
}

#[test]
fn consecutive_losses_halves_position() {
    let pct = compute_position_pct(30.0, 2, 0.0);
    assert!((pct - 40.0).abs() < 0.001, "expected 40.0, got {}", pct);
}

#[test]
fn single_loss_does_not_reduce() {
    let pct = compute_position_pct(30.0, 1, 0.0);
    assert!((pct - 80.0).abs() < 0.001, "expected 80.0, got {}", pct);
}

#[test]
fn high_funding_rate_halves_position() {
    let pct = compute_position_pct(30.0, 0, 0.002);
    assert!((pct - 40.0).abs() < 0.001, "expected 40.0, got {}", pct);
}

#[test]
fn negative_funding_rate_halves_position() {
    let pct = compute_position_pct(30.0, 0, -0.002);
    assert!((pct - 40.0).abs() < 0.001, "expected 40.0, got {}", pct);
}

#[test]
fn low_funding_rate_no_effect() {
    let pct = compute_position_pct(30.0, 0, 0.0005);
    assert!((pct - 80.0).abs() < 0.001, "expected 80.0, got {}", pct);
}

#[test]
fn losses_and_funding_combined() {
    let pct = compute_position_pct(30.0, 2, 0.002);
    assert!((pct - 20.0).abs() < 0.001, "expected 20.0, got {}", pct);
}

#[test]
fn all_reducers_hit_minimum_clamp() {
    let pct = compute_position_pct(10.0, 2, 0.01);
    assert!((pct - 10.0).abs() < 0.001, "expected clamped 10.0, got {}", pct);
}

#[test]
fn zero_adx_base_40() {
    let pct = compute_position_pct(0.0, 0, 0.0);
    assert!((pct - 40.0).abs() < 0.001, "expected 40.0, got {}", pct);
}

#[test]
fn boundary_adx_25() {
    assert!((compute_position_pct(25.0, 0, 0.0) - 80.0).abs() < 0.001);
    assert!((compute_position_pct(24.99, 0, 0.0) - 60.0).abs() < 0.001);
}

#[test]
fn boundary_adx_20() {
    assert!((compute_position_pct(20.0, 0, 0.0) - 60.0).abs() < 0.001);
    assert!((compute_position_pct(19.99, 0, 0.0) - 40.0).abs() < 0.001);
}

#[test]
fn clamp_upper_bound_100() {
    let pct = compute_position_pct(30.0, 0, 0.0);
    assert!(pct <= 100.0, "should not exceed 100.0, got {}", pct);
}

#[test]
fn clamp_lower_bound_10() {
    let pct = compute_position_pct(10.0, 5, 0.01);
    assert!((pct - 10.0).abs() < 0.001, "should clamp to 10.0, got {}", pct);
}
