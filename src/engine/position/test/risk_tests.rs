//! 风控模块外部集成测试
//!
//! 测试 RiskChecker 与 types 模块的交互。

use super::common::*;
use super::super::config::RiskConfig;
use super::super::risk::{DrawdownAction, RiskAlertInfo, RiskChecker};
use super::super::types::*;

// ============================================================================
// 辅助函数
// ============================================================================

/// 使用自定义 RiskConfig 创建 RiskChecker。
fn make_checker(config: RiskConfig) -> RiskChecker {
    RiskChecker::new(config)
}

/// 使用默认 RiskConfig 创建 RiskChecker。
fn default_checker() -> RiskChecker {
    make_checker(RiskConfig::default())
}

// ============================================================================
// 杠杆检查 (3 tests)
// ============================================================================

#[test]
fn test_leverage_within_limit() {
    // max_leverage=20, 请求 leverage=10 -> Ok
    let config = RiskConfig {
        max_leverage: 20,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let result = checker.check_open_position(&[], "BTCUSDT", 1000.0, 10, 10000.0);
    assert!(result.is_ok(), "leverage=10 应在 max_leverage=20 范围内");
}

#[test]
fn test_leverage_at_limit() {
    // max_leverage=20, 请求 leverage=20 -> Ok
    let config = RiskConfig {
        max_leverage: 20,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let result = checker.check_open_position(&[], "BTCUSDT", 1000.0, 20, 10000.0);
    assert!(result.is_ok(), "leverage=20 应刚好等于 max_leverage=20，允许通过");
}

#[test]
fn test_leverage_exceeds_limit() {
    // max_leverage=20, 请求 leverage=21 -> Err
    let config = RiskConfig {
        max_leverage: 20,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let result = checker.check_open_position(&[], "BTCUSDT", 1000.0, 21, 10000.0);
    assert!(result.is_err(), "leverage=21 超过 max_leverage=20，应被拒绝");
}

// ============================================================================
// 单品种仓位上限 (3 tests)
// ============================================================================

#[test]
fn test_single_symbol_within_limit() {
    // equity=10000, max_position_per_symbol_pct=1.0, 已有仓位 margin=3000, 新仓 margin=2000 -> Ok
    let config = RiskConfig {
        max_position_per_symbol_pct: 1.0,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    // 创建一个 margin=3000 的仓位: size * entry_price / leverage = 3000
    // 例如 size=3, entry_price=10000, leverage=10 -> margin = 30000/10 = 3000
    let pos = make_position("BTCUSDT", 3.0, 10000.0, 10);
    assert!((pos.margin - 3000.0).abs() < 0.01, "margin 应为 3000");

    let positions: Vec<&Position> = vec![&pos];

    // 新仓 margin = new_amount / new_leverage = 2000
    // 例如 new_amount=20000, leverage=10 -> margin = 2000
    let result = checker.check_open_position(&positions, "BTCUSDT", 20000.0, 10, 10000.0);
    assert!(
        result.is_ok(),
        "已有 3000 + 新仓 2000 = 5000 <= 10000*1.0=10000，应通过"
    );
}

#[test]
fn test_single_symbol_at_limit() {
    // equity=10000, 已有 8000, 新仓 2000 -> Ok (刚好 100%)
    let config = RiskConfig {
        max_position_per_symbol_pct: 1.0,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    // margin=8000: size=8, entry_price=10000, leverage=10 -> 80000/10 = 8000
    let pos = make_position("BTCUSDT", 8.0, 10000.0, 10);
    let positions: Vec<&Position> = vec![&pos];

    // 新仓 margin = 20000/10 = 2000
    let result = checker.check_open_position(&positions, "BTCUSDT", 20000.0, 10, 10000.0);
    assert!(
        result.is_ok(),
        "已有 8000 + 新仓 2000 = 10000 <= 10000*1.0=10000，刚好等于上限，应通过"
    );
}

#[test]
fn test_single_symbol_exceeds_limit() {
    // equity=10000, 已有 8000, 新仓 3000 -> Err
    let config = RiskConfig {
        max_position_per_symbol_pct: 1.0,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    // margin=8000: size=8, entry_price=10000, leverage=10 -> 8000
    let pos = make_position("BTCUSDT", 8.0, 10000.0, 10);
    let positions: Vec<&Position> = vec![&pos];

    // 新仓 margin = 30000/10 = 3000
    let result = checker.check_open_position(&positions, "BTCUSDT", 30000.0, 10, 10000.0);
    assert!(
        result.is_err(),
        "已有 8000 + 新仓 3000 = 11000 > 10000*1.0=10000，应被拒绝"
    );
}

// ============================================================================
// 总仓位上限 (3 tests)
// ============================================================================

#[test]
fn test_total_position_within_limit() {
    // equity=10000, max_total_position_pct=3.0, 已有总仓位 margin=20000, 新仓 margin=5000 -> Ok
    let config = RiskConfig {
        max_total_position_pct: 3.0,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    // margin=20000: size=20, entry_price=10000, leverage=10 -> 20000
    let pos = make_position("BTCUSDT", 20.0, 10000.0, 10);
    let positions: Vec<&Position> = vec![&pos];

    // 新仓 margin = 50000/10 = 5000
    let result = checker.check_open_position(&positions, "ETHUSDT", 50000.0, 10, 10000.0);
    assert!(
        result.is_ok(),
        "已有 20000 + 新仓 5000 = 25000 <= 10000*3.0=30000，应通过"
    );
}

#[test]
fn test_total_position_exceeds_limit() {
    // equity=10000, max_total_position_pct=3.0, 已有 25000, 新仓 6000 -> Err
    let config = RiskConfig {
        max_total_position_pct: 3.0,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    // margin=25000: size=25, entry_price=10000, leverage=10 -> 25000
    let pos = make_position("BTCUSDT", 25.0, 10000.0, 10);
    let positions: Vec<&Position> = vec![&pos];

    // 新仓 margin = 60000/10 = 6000
    let result = checker.check_open_position(&positions, "ETHUSDT", 60000.0, 10, 10000.0);
    assert!(
        result.is_err(),
        "已有 25000 + 新仓 6000 = 31000 > 10000*3.0=30000，应被拒绝"
    );
}

#[test]
fn test_total_position_multi_symbol() {
    // 多个 symbol 的仓位 margin 之和超过总上限 -> Err
    let config = RiskConfig {
        max_total_position_pct: 3.0,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    // BTC margin=15000: size=15, entry_price=10000, leverage=10 -> 15000
    let pos_btc = make_position("BTCUSDT", 15.0, 10000.0, 10);
    // ETH margin=15000: size=15, entry_price=10000, leverage=10 -> 15000
    let pos_eth = make_position("ETHUSDT", 15.0, 10000.0, 10);

    let positions: Vec<&Position> = vec![&pos_btc, &pos_eth];

    // 总已有 = 30000, 上限 = 10000*3.0=30000
    // 新仓 margin = 1000/10 = 100
    let result = checker.check_open_position(&positions, "SOLUSDT", 1000.0, 10, 10000.0);
    assert!(
        result.is_err(),
        "已有 30000 + 新仓 100 = 30100 > 30000，应被拒绝"
    );
}

// ============================================================================
// 单笔下单上限 (2 tests)
// ============================================================================

#[test]
fn test_order_amount_within_limit() {
    // max_order_amount_pct=0.3, equity=10000, max_amount=3000, order amount=2000 -> Ok
    let config = RiskConfig {
        max_order_amount_pct: 0.3,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let result = checker.check_place_order(&[], "BTCUSDT", 2000.0, 10000.0);
    assert!(
        result.is_ok(),
        "下单金额 2000 <= 10000*0.3=3000，应通过"
    );
}

#[test]
fn test_order_amount_exceeds_limit() {
    // max_order_amount_pct=0.3, equity=10000, max_amount=3000, order amount=4000 -> Err
    let config = RiskConfig {
        max_order_amount_pct: 0.3,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let result = checker.check_place_order(&[], "BTCUSDT", 4000.0, 10000.0);
    assert!(
        result.is_err(),
        "下单金额 4000 > 10000*0.3=3000，应被拒绝"
    );
}

// ============================================================================
// 连续亏损 (4 tests)
// ============================================================================

#[test]
fn test_consecutive_losses_reset_on_profit() {
    // 连亏 4 次后盈利 1 次 -> count=0
    let mut checker = default_checker();

    for _ in 0..4 {
        checker.record_trade_result(-1.0);
    }
    assert_eq!(checker.consecutive_losses(), 4, "应记录 4 次连续亏损");

    checker.record_trade_result(10.0);
    assert_eq!(
        checker.consecutive_losses(),
        0,
        "盈利后连续亏损计数应重置为 0"
    );
}

#[test]
fn test_consecutive_losses_trigger() {
    // 连亏 5 次 -> should_reduce_position() = true
    let mut checker = default_checker();

    for _ in 0..5 {
        checker.record_trade_result(-1.0);
    }
    assert_eq!(checker.consecutive_losses(), 5);
    assert!(
        checker.should_reduce_position(),
        "连续亏损 5 次达到阈值，should_reduce_position 应为 true"
    );
}

#[test]
fn test_consecutive_losses_blocks_open() {
    // 连亏 5 次后尝试开仓 -> Err
    let mut checker = default_checker();

    for _ in 0..5 {
        checker.record_trade_result(-1.0);
    }

    let result = checker.check_open_position(&[], "BTCUSDT", 1000.0, 5, 10000.0);
    assert!(
        result.is_err(),
        "连续亏损 5 次后开仓应被拒绝"
    );
}

#[test]
fn test_consecutive_losses_below_threshold() {
    // 连亏 4 次 -> should_reduce_position() = false, 开仓 Ok
    let mut checker = default_checker();

    for _ in 0..4 {
        checker.record_trade_result(-1.0);
    }
    assert_eq!(checker.consecutive_losses(), 4);
    assert!(
        !checker.should_reduce_position(),
        "连续亏损 4 次未达到阈值 5，should_reduce_position 应为 false"
    );

    let result = checker.check_open_position(&[], "BTCUSDT", 1000.0, 5, 10000.0);
    assert!(
        result.is_ok(),
        "连续亏损 4 次未达到阈值，开仓应通过"
    );
}

// ============================================================================
// 回撤分级 (4 tests)
// ============================================================================

#[test]
fn test_drawdown_normal() {
    // peak=10000, current=9500, max_dd=0.15 -> drawdown=5% < 7.5% -> Normal (None)
    let config = RiskConfig {
        max_drawdown_pct: 0.15,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let action = checker.check_drawdown(10000.0, 9500.0);
    // drawdown = (10000-9500)/10000 = 5%
    // 5% < 0.15*0.5 = 7.5% -> None (Normal)
    assert!(
        action.is_none(),
        "回撤 5% < 7.5%，应为 None (Normal)"
    );
}

#[test]
fn test_drawdown_warning() {
    // peak=10000, current=9000, max_dd=0.15 -> drawdown=10% >= 7.5% -> Warning
    let config = RiskConfig {
        max_drawdown_pct: 0.15,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let action = checker.check_drawdown(10000.0, 9000.0);
    // drawdown = (10000-9000)/10000 = 10%
    // 10% >= 7.5% (0.15*0.5) 且 10% < 11.25% (0.15*0.75) -> Warning
    assert_eq!(
        action,
        Some(DrawdownAction::Warning),
        "回撤 10% >= 7.5% 且 < 11.25%，应为 Warning"
    );
}

#[test]
fn test_drawdown_pause() {
    // peak=10000, current=8700, max_dd=0.15 -> drawdown=13% >= 11.25% 且 < 15% -> Pause
    let config = RiskConfig {
        max_drawdown_pct: 0.15,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let action = checker.check_drawdown(10000.0, 8700.0);
    // drawdown = (10000-8700)/10000 = 13%
    // 13% >= 11.25% (0.15*0.75) 且 13% < 15% (0.15) -> Pause
    assert_eq!(
        action,
        Some(DrawdownAction::Pause),
        "回撤 13% >= 11.25% 且 < 15%，应为 Pause"
    );
}

#[test]
fn test_drawdown_close_all() {
    // peak=10000, current=8000, max_dd=0.15 -> drawdown=20% >= 15% -> CloseAll
    let config = RiskConfig {
        max_drawdown_pct: 0.15,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let action = checker.check_drawdown(10000.0, 8000.0);
    // drawdown = (10000-8000)/10000 = 20%
    // 20% >= 15% (0.15) -> CloseAll
    assert_eq!(
        action,
        Some(DrawdownAction::CloseAll),
        "回撤 20% >= 15%，应为 CloseAll"
    );
}

// ============================================================================
// 强平预警 (4 tests)
// ============================================================================

#[test]
fn test_liquidation_far_from_price() {
    // current=100, liq=50, buffer=0.2 -> distance=50% > 20% -> None
    let config = RiskConfig {
        liquidation_buffer_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let pos = make_position_with_liquidation("BTCUSDT", 1.0, 100.0, 1, Some(50.0), 100.0);
    let result = checker.check_liquidation(&pos);
    // distance = |100-50|/100 = 50% > 20% -> None
    assert!(
        result.is_none(),
        "距离强平 50% > 20% buffer，应返回 None"
    );
}

#[test]
fn test_liquidation_near_price() {
    // current=100, liq=85, buffer=0.2 -> distance=15% < 20% -> Some(15%)
    let config = RiskConfig {
        liquidation_buffer_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let pos = make_position_with_liquidation("BTCUSDT", 1.0, 100.0, 1, Some(85.0), 100.0);
    let result = checker.check_liquidation(&pos);
    // distance = |100-85|/100 = 15% < 20% -> Some(15%)
    assert!(result.is_some(), "距离强平 15% < 20% buffer，应返回 Some");
    let distance = result.unwrap();
    assert!(
        (distance - 0.15).abs() < 0.001,
        "距离应为 15%，实际为 {}",
        distance
    );
}

#[test]
fn test_liquidation_no_liquidation_price() {
    // liquidation_price = None -> None
    let config = RiskConfig {
        liquidation_buffer_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let pos = make_position_with_liquidation("BTCUSDT", 1.0, 100.0, 1, None, 100.0);
    let result = checker.check_liquidation(&pos);
    assert!(
        result.is_none(),
        "无强平价时应返回 None"
    );
}

#[test]
fn test_liquidation_zero_current_price() {
    // current_price = 0 -> None (避免除零)
    let config = RiskConfig {
        liquidation_buffer_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let pos = make_position_with_liquidation("BTCUSDT", 1.0, 100.0, 1, Some(50.0), 0.0);
    let result = checker.check_liquidation(&pos);
    assert!(
        result.is_none(),
        "当前价格为 0 时应返回 None，避免除零"
    );
}

// ============================================================================
// 资金费率检查 (3 tests)
// ============================================================================

#[test]
fn test_funding_rate_normal() {
    // rate=0.0005, threshold=0.001 -> None
    let config = RiskConfig {
        funding_rate_threshold: 0.001,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let result = checker.check_funding_rate("BTCUSDT", 0.0005);
    assert!(
        result.is_none(),
        "费率 0.0005 < 阈值 0.001，应返回 None"
    );
}

#[test]
fn test_funding_rate_exceeds_threshold() {
    // rate=0.002, threshold=0.001 -> Some(RiskAlertInfo), severity="warning"
    let config = RiskConfig {
        funding_rate_threshold: 0.001,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let result = checker.check_funding_rate("BTCUSDT", 0.002);
    assert!(result.is_some(), "费率 0.002 > 阈值 0.001，应返回 Some");

    let alert = result.unwrap();
    assert_eq!(alert.symbol, "BTCUSDT");
    assert_eq!(alert.alert_type, "funding_rate");
    assert_eq!(alert.severity, "critical", "费率 0.002 >= 2*0.001=0.002，应为 critical");
}

#[test]
fn test_funding_rate_critical() {
    // rate=0.003, threshold=0.001 -> severity="critical" (超过 2 倍)
    let config = RiskConfig {
        funding_rate_threshold: 0.001,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let result = checker.check_funding_rate("BTCUSDT", 0.003);
    assert!(result.is_some(), "费率 0.003 > 阈值 0.001，应返回 Some");

    let alert = result.unwrap();
    assert_eq!(alert.severity, "critical", "费率 0.003 > 2*0.001=0.002，应为 critical");
}

// ============================================================================
// 边界条件 (3 tests)
// ============================================================================

#[test]
fn test_zero_equity_blocks_all() {
    // equity=0, 任何开仓都应被拒绝
    let checker = default_checker();

    // 杠杆检查本身不依赖 equity，但单品种仓位上限检查会：
    // symbol_limit = 0 * 1.0 = 0，任何新仓 margin > 0 都会被拒绝
    let result = checker.check_open_position(&[], "BTCUSDT", 1000.0, 5, 0.0);
    assert!(
        result.is_err(),
        "equity=0 时开仓应被拒绝"
    );
}

#[test]
fn test_empty_positions_allows_open() {
    // 没有现有仓位，开仓应通过
    let checker = default_checker();

    let positions: Vec<&Position> = vec![];
    let result = checker.check_open_position(&positions, "BTCUSDT", 1000.0, 5, 10000.0);
    assert!(
        result.is_ok(),
        "没有现有仓位时，正常开仓应通过"
    );
}

#[test]
fn test_custom_risk_config() {
    // 自定义 RiskConfig 参数验证
    let config = RiskConfig {
        max_leverage: 10,
        max_position_per_symbol_pct: 0.5,
        max_total_position_pct: 2.0,
        max_order_amount_pct: 0.1,
        max_drawdown_pct: 0.10,
        funding_rate_threshold: 0.0005,
        liquidation_buffer_pct: 0.15,
        max_consecutive_losses: 3,
    };
    let checker = make_checker(config.clone());

    // 验证 max_leverage=10
    let result = checker.check_open_position(&[], "BTCUSDT", 1000.0, 10, 10000.0);
    assert!(result.is_ok(), "leverage=10 应等于自定义 max_leverage=10");

    let result = checker.check_open_position(&[], "BTCUSDT", 1000.0, 11, 10000.0);
    assert!(result.is_err(), "leverage=11 应超过自定义 max_leverage=10");

    // 验证 max_consecutive_losses=3
    let mut checker_mut = make_checker(config.clone());
    for _ in 0..3 {
        checker_mut.record_trade_result(-1.0);
    }
    assert!(
        checker_mut.should_reduce_position(),
        "自定义 max_consecutive_losses=3，连亏 3 次应触发减仓"
    );

    // 验证 max_drawdown_pct=0.10
    let action = checker.check_drawdown(10000.0, 9000.0);
    // drawdown = 10% >= 0.10 -> CloseAll
    assert_eq!(
        action,
        Some(DrawdownAction::CloseAll),
        "自定义 max_drawdown_pct=0.10，回撤 10% 应触发 CloseAll"
    );

    // 验证 funding_rate_threshold=0.0005
    let result = checker.check_funding_rate("BTCUSDT", 0.0006);
    assert!(
        result.is_some(),
        "自定义 funding_rate_threshold=0.0005，费率 0.0006 应触发告警"
    );

    // 验证 liquidation_buffer_pct=0.15
    let pos = make_position_with_liquidation("BTCUSDT", 1.0, 100.0, 1, Some(88.0), 100.0);
    let result = checker.check_liquidation(&pos);
    // distance = |100-88|/100 = 12% < 15% -> Some
    assert!(
        result.is_some(),
        "自定义 liquidation_buffer_pct=0.15，距离 12% < 15% 应触发预警"
    );
}

// ============================================================
// 补充边界场景 (5 tests)
// ============================================================

#[test]
fn test_check_place_order_with_existing_position() {
    // check_place_order: max_amount = total_equity * max_order_amount_pct
    // equity=10000, max_order_amount_pct=0.3 -> max_amount = 3000
    // order amount=1000 -> Ok
    // order amount=2000 -> Ok
    // order amount=4000 -> Err
    let config = RiskConfig {
        max_order_amount_pct: 0.3,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let pos = make_position("BTCUSDT", 5.0, 10000.0, 10);
    let positions: Vec<&Position> = vec![&pos];

    // amount=1000 <= 3000 -> Ok
    let result = checker.check_place_order(&positions, "BTCUSDT", 1000.0, 10000.0);
    assert!(result.is_ok(), "下单金额 1000 <= 3000，应通过");

    // amount=2000 <= 3000 -> Ok
    let result2 = checker.check_place_order(&positions, "BTCUSDT", 2000.0, 10000.0);
    assert!(result2.is_ok(), "下单金额 2000 <= 3000，应通过");

    // amount=4000 > 3000 -> Err
    let result3 = checker.check_place_order(&positions, "BTCUSDT", 4000.0, 10000.0);
    assert!(result3.is_err(), "下单金额 4000 > 3000，应被拒绝");
}

#[test]
fn test_check_drawdown_zero_peak() {
    // peak=0, current=0 -> drawdown=0 (不 panic)
    let checker = default_checker();
    let action = checker.check_drawdown(0.0, 0.0);
    assert!(action.is_none());
}

#[test]
fn test_check_drawdown_negative_equity() {
    // peak=10000, current=-5000 -> drawdown = (10000-(-5000))/10000 = 1.5 -> CloseAll
    let checker = default_checker();
    let action = checker.check_drawdown(10000.0, -5000.0);
    assert_eq!(
        action,
        Some(DrawdownAction::CloseAll),
        "回撤 150% >= 15%，应为 CloseAll"
    );
}

#[test]
fn test_funding_rate_negative() {
    // 负费率也应触发告警（绝对值超过阈值）
    // rate=-0.002, threshold=0.001 -> Some, severity="critical"
    let config = RiskConfig {
        funding_rate_threshold: 0.001,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let result = checker.check_funding_rate("BTCUSDT", -0.002);
    assert!(result.is_some(), "费率 -0.002 的绝对值超过阈值 0.001，应返回 Some");

    let alert = result.unwrap();
    assert_eq!(alert.symbol, "BTCUSDT");
    assert_eq!(alert.alert_type, "funding_rate");
    assert_eq!(alert.severity, "critical", "费率绝对值 0.002 >= 2*0.001=0.002，应为 critical");
}

#[test]
fn test_funding_rate_exactly_at_threshold() {
    // rate=0.001, threshold=0.001 -> 不触发（> 而非 >=）
    let config = RiskConfig {
        funding_rate_threshold: 0.001,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    assert!(checker.check_funding_rate("BTCUSDT", 0.001).is_none());
}

// ============================================================
// 更多边界场景
// ============================================================

#[test]
fn test_leverage_zero_rejected() {
    let config = RiskConfig {
        max_leverage: 20,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let result = checker.check_open_position(&[], "BTCUSDT", 1000.0, 0, 10000.0);
    assert!(
        result.is_err(),
        "leverage=0 会导致 margin=inf，应被拒绝"
    );
}

#[test]
fn test_leverage_one() {
    let config = RiskConfig {
        max_leverage: 20,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let result = checker.check_open_position(&[], "BTCUSDT", 1000.0, 1, 10000.0);
    assert!(result.is_ok(), "leverage=1 应在 max_leverage=20 范围内");
}

#[test]
fn test_check_open_position_zero_amount() {
    let checker = default_checker();
    let result = checker.check_open_position(&[], "BTCUSDT", 0.0, 5, 10000.0);
    assert!(result.is_ok(), "amount=0 时 margin=0，应通过所有仓位上限检查");
}

#[test]
fn test_check_open_position_different_symbols() {
    let config = RiskConfig {
        max_position_per_symbol_pct: 0.5,
        max_total_position_pct: 2.0,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let pos_btc = make_position("BTCUSDT", 5.0, 10000.0, 10);
    let positions: Vec<&Position> = vec![&pos_btc];

    let result = checker.check_open_position(&positions, "ETHUSDT", 1000.0, 5, 10000.0);
    assert!(
        result.is_ok(),
        "不同 symbol 的仓位不影响单品种上限检查"
    );
}

#[test]
fn test_check_place_order_zero_equity() {
    let config = RiskConfig {
        max_order_amount_pct: 0.3,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let result = checker.check_place_order(&[], "BTCUSDT", 1.0, 0.0);
    assert!(
        result.is_err(),
        "equity=0 时 max_amount=0，任何下单都应被拒绝"
    );
}

#[test]
fn test_check_place_order_zero_amount() {
    let config = RiskConfig {
        max_order_amount_pct: 0.3,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let result = checker.check_place_order(&[], "BTCUSDT", 0.0, 10000.0);
    assert!(
        result.is_ok(),
        "amount=0 <= max_amount=3000，应通过"
    );
}

#[test]
fn test_check_place_order_amount_at_limit() {
    let config = RiskConfig {
        max_order_amount_pct: 0.3,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let result = checker.check_place_order(&[], "BTCUSDT", 3000.0, 10000.0);
    assert!(
        result.is_ok(),
        "amount=3000 == max_amount=3000，应通过（<= 判断）"
    );
}

#[test]
fn test_drawdown_exactly_at_50pct_threshold() {
    let config = RiskConfig {
        max_drawdown_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let action = checker.check_drawdown(10000.0, 9000.0);
    assert_eq!(
        action,
        Some(DrawdownAction::Warning),
        "回撤 10% 恰好等于 0.2*0.5=10%，应为 Warning"
    );
}

#[test]
fn test_drawdown_exactly_at_75pct_threshold() {
    let config = RiskConfig {
        max_drawdown_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let action = checker.check_drawdown(10000.0, 8501.0);
    assert_eq!(
        action,
        Some(DrawdownAction::Warning),
        "回撤 14.99% 略低于 0.2*0.75=15%，应为 Warning"
    );
    let action2 = checker.check_drawdown(10000.0, 8499.0);
    assert_eq!(
        action2,
        Some(DrawdownAction::Pause),
        "回撤 15.01% 略高于 0.2*0.75=15%，应为 Pause"
    );
}

#[test]
fn test_drawdown_exactly_at_max() {
    let config = RiskConfig {
        max_drawdown_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let action = checker.check_drawdown(10000.0, 8000.0);
    assert_eq!(
        action,
        Some(DrawdownAction::CloseAll),
        "回撤 20% 恰好等于 max_drawdown_pct=0.2，应为 CloseAll"
    );
}

#[test]
fn test_drawdown_profit_no_drawdown() {
    let checker = default_checker();
    let action = checker.check_drawdown(10000.0, 12000.0);
    assert!(
        action.is_none(),
        "current > peak 时无回撤，应返回 None"
    );
}

#[test]
fn test_drawdown_peak_equals_current() {
    let checker = default_checker();
    let action = checker.check_drawdown(10000.0, 10000.0);
    assert!(
        action.is_none(),
        "peak == current 时回撤为 0，应返回 None"
    );
}

#[test]
fn test_liquidation_price_equals_current() {
    let config = RiskConfig {
        liquidation_buffer_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let pos = make_position_with_liquidation("BTCUSDT", 1.0, 100.0, 1, Some(100.0), 100.0);
    let result = checker.check_liquidation(&pos);
    assert!(
        result.is_some(),
        "强平价等于当前价格时距离为 0%，应触发预警"
    );
    let distance = result.unwrap();
    assert!(
        distance.abs() < 1e-8,
        "距离应为 0，实际为 {}",
        distance
    );
}

#[test]
fn test_liquidation_price_above_current() {
    let config = RiskConfig {
        liquidation_buffer_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let pos = make_position_with_liquidation("BTCUSDT", 1.0, 100.0, 1, Some(110.0), 100.0);
    let result = checker.check_liquidation(&pos);
    assert!(
        result.is_some(),
        "强平价高于当前价格（Short 仓位场景），距离 10% < 20%，应触发预警"
    );
}

#[test]
fn test_liquidation_zero_liq_price() {
    let config = RiskConfig {
        liquidation_buffer_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let pos = make_position_with_liquidation("BTCUSDT", 1.0, 100.0, 1, Some(0.0), 100.0);
    let result = checker.check_liquidation(&pos);
    assert!(
        result.is_none(),
        "强平价为 0 时应返回 None，避免除零"
    );
}

#[test]
fn test_liquidation_exactly_at_buffer() {
    let config = RiskConfig {
        liquidation_buffer_pct: 0.2,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let pos = make_position_with_liquidation("BTCUSDT", 1.0, 100.0, 1, Some(80.0), 100.0);
    let result = checker.check_liquidation(&pos);
    assert!(
        result.is_some(),
        "距离恰好等于 buffer 20%，应触发预警（<= 判断）"
    );
}

#[test]
fn test_consecutive_losses_profit_then_loss() {
    let mut checker = default_checker();
    checker.record_trade_result(-1.0);
    checker.record_trade_result(-1.0);
    checker.record_trade_result(5.0);
    assert_eq!(checker.consecutive_losses(), 0, "盈利后重置为 0");
    checker.record_trade_result(-1.0);
    assert_eq!(checker.consecutive_losses(), 1, "盈利后再亏损从 1 开始计数");
}

#[test]
fn test_consecutive_losses_zero_pnl_resets() {
    let mut checker = default_checker();
    checker.record_trade_result(-1.0);
    checker.record_trade_result(-1.0);
    assert_eq!(checker.consecutive_losses(), 2);
    checker.record_trade_result(0.0);
    assert_eq!(
        checker.consecutive_losses(),
        0,
        "pnl=0 应视为非亏损，重置计数"
    );
}

#[test]
fn test_funding_rate_exactly_at_2x_threshold() {
    let config = RiskConfig {
        funding_rate_threshold: 0.001,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let result = checker.check_funding_rate("BTCUSDT", 0.002);
    assert!(result.is_some());
    let alert = result.unwrap();
    assert_eq!(
        alert.severity,
        "critical",
        "费率恰好等于 2 倍阈值，应为 critical（>= 判断）"
    );
}

#[test]
fn test_funding_rate_between_1x_and_2x() {
    let config = RiskConfig {
        funding_rate_threshold: 0.001,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);
    let result = checker.check_funding_rate("BTCUSDT", 0.0015);
    assert!(result.is_some());
    let alert = result.unwrap();
    assert_eq!(
        alert.severity,
        "warning",
        "费率 0.0015 在 1x-2x 阈值之间，应为 warning"
    );
}

#[test]
fn test_check_open_position_multiple_checks_combined() {
    let config = RiskConfig {
        max_leverage: 10,
        max_position_per_symbol_pct: 0.5,
        max_total_position_pct: 1.5,
        max_consecutive_losses: 3,
        ..RiskConfig::default()
    };
    let mut checker = make_checker(config);

    for _ in 0..3 {
        checker.record_trade_result(-1.0);
    }

    let result = checker.check_open_position(&[], "BTCUSDT", 100.0, 5, 10000.0);
    assert!(
        result.is_err(),
        "即使杠杆和仓位都合规，连续亏损也应阻止开仓"
    );
}

#[test]
fn test_total_position_at_limit() {
    let config = RiskConfig {
        max_total_position_pct: 3.0,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let pos = make_position("BTCUSDT", 25.0, 10000.0, 10);
    let positions: Vec<&Position> = vec![&pos];

    let result = checker.check_open_position(&positions, "ETHUSDT", 50000.0, 10, 10000.0);
    assert!(
        result.is_ok(),
        "已有 25000 + 新仓 5000 = 30000 <= 10000*3.0=30000，恰好等于上限，应通过"
    );
}

#[test]
fn test_single_symbol_no_existing_position() {
    let config = RiskConfig {
        max_position_per_symbol_pct: 0.5,
        ..RiskConfig::default()
    };
    let checker = make_checker(config);

    let pos_btc = make_position("BTCUSDT", 5.0, 10000.0, 10);
    let positions: Vec<&Position> = vec![&pos_btc];

    let result = checker.check_open_position(&positions, "ETHUSDT", 5000.0, 10, 10000.0);
    assert!(
        result.is_ok(),
        "ETHUSDT 无已有仓位，新仓 margin=500 <= 10000*0.5=5000，应通过"
    );
}
