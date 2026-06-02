/**
 * 测试 strategy::compute_leverage 杠杆计算
 * - 现货：始终返回 1
 * - 合约 ADX>=30：5x
 * - 合约 ADX>=20：3x
 * - 合约 ADX<20：2x
 */
use crate::bot::auto_trade::strategy::compute_leverage;

#[test]
fn spot_always_returns_1() {
    assert_eq!(compute_leverage(35.0, true), 1);
    assert_eq!(compute_leverage(25.0, true), 1);
    assert_eq!(compute_leverage(10.0, true), 1);
}

#[test]
fn perpetual_high_adx_returns_5() {
    assert_eq!(compute_leverage(30.0, false), 5);
    assert_eq!(compute_leverage(45.0, false), 5);
    assert_eq!(compute_leverage(100.0, false), 5);
}

#[test]
fn perpetual_medium_adx_returns_3() {
    assert_eq!(compute_leverage(20.0, false), 3);
    assert_eq!(compute_leverage(25.0, false), 3);
    assert_eq!(compute_leverage(29.9, false), 3);
}

#[test]
fn perpetual_low_adx_returns_2() {
    assert_eq!(compute_leverage(0.0, false), 2);
    assert_eq!(compute_leverage(10.0, false), 2);
    assert_eq!(compute_leverage(19.9, false), 2);
}

#[test]
fn boundary_adx_30() {
    assert_eq!(compute_leverage(30.0, false), 5);
    assert_eq!(compute_leverage(29.99, false), 3);
}

#[test]
fn boundary_adx_20() {
    assert_eq!(compute_leverage(20.0, false), 3);
    assert_eq!(compute_leverage(19.99, false), 2);
}
