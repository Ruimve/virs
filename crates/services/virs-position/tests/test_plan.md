# virs-position 测试用例文档

> Crate: `crates/services/virs-position`
> 状态: **41 个测试全部通过** (31 单元 + 10 集成)

---

## 1. 单元测试用例

### 1.1 risk_tests.rs — 风控检查 (18)

| ID | 测试函数 | 描述 |
|----|---------|------|
| R1.1 | `r1_1_check_funding_rate_normal` | 费率 < 阈值 → None |
| R1.2 | `r1_2_check_funding_rate_warning` | 费率 > 阈值 → warning |
| R1.3 | `r1_3_check_funding_rate_critical` | 费率 > 阈值*2 → critical |
| R1.4 | `r1_4_check_funding_rate_negative` | 负费率, |rate| > 阈值 → warning |
| R2.1 | `r2_1_check_drawdown_normal` | 回撤 < max_dd*0.5 → None |
| R2.2 | `r2_2_check_drawdown_warning` | 回撤 ≥ max_dd*0.5 → Warning |
| R2.3 | `r2_3_check_drawdown_pause` | 回撤 ≥ max_dd*0.75 → Pause |
| R2.4 | `r2_4_check_drawdown_close_all` | 回撤 ≥ max_dd → CloseAll |
| R2.5 | `r2_5_check_drawdown_zero_peak` | peak=0 → None |
| R3.1 | `r3_1_check_liquidation_none` | 无强平价 → None |
| R3.2 | `r3_2_check_liquidation_far` | 距离 > buffer → None |
| R3.3 | `r3_3_check_liquidation_close` | 距离 ≤ buffer → Some(pct) |
| R3.4 | `r3_4_check_liquidation_zero_price` | current=0 → None |
| R4.1 | `r4_1_calc_symbol_exposure_empty` | 空列表 → 0.0 |
| R4.2 | `r4_2_calc_symbol_exposure_single` | 1个匹配仓位 → margin |
| R4.3 | `r4_3_calc_symbol_exposure_multi_symbol` | 多品种 → 仅匹配品种之和 |
| R5.1 | `r5_1_calc_total_exposure_empty` | 空列表 → 0.0 |
| R5.2 | `r5_2_calc_total_exposure_multi` | 多仓位 → 保证金之和 |

### 1.2 tracker_tests.rs — 盈亏追踪 (13)

| ID | 测试函数 | 描述 |
|----|---------|------|
| P1.1 | `p1_1_calc_unrealized_pnl_empty` | 空列表 → 0.0 |
| P1.2 | `p1_2_calc_unrealized_pnl_long_profit` | long, price>entry → 正盈亏 |
| P1.3 | `p1_3_calc_unrealized_pnl_long_loss` | long, price<entry → 负盈亏 |
| P1.4 | `p1_4_calc_unrealized_pnl_short_profit` | short, price<entry → 正盈亏 |
| P1.5 | `p1_5_calc_unrealized_pnl_short_loss` | short, price>entry → 负盈亏 |
| P1.7 | `p1_7_calc_unrealized_pnl_no_price` | 无当前价格 → 使用 pos.current_price |
| P2.1 | `p2_1_calc_drawdown_pct_zero_peak` | peak=0 → 0.0 |
| P2.2 | `p2_2_calc_drawdown_pct_no_drawdown` | equity > peak → 负值 |
| P2.3 | `p2_3_calc_drawdown_pct_partial` | 10% 回撤 → 0.1 |
| P2.4 | `p2_4_calc_drawdown_pct_full` | equity=0 → 1.0 |
| P3.1 | `p3_1_snapshot_equity_and_drawdown` | 交易后快照 equity/peak/drawdown |
| P3.2 | `p3_2_snapshot_with_unrealized` | 含未实现盈亏的快照 equity |

---

## 2. 集成测试用例

### integration_tests.rs (10)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_funding_rate_then_drawdown` | 费率告警 → 回撤检查链路 |
| INT-1.2 | `int_1_2_drawdown_escalation` | 回撤从 None → Warning → Pause → CloseAll |
| INT-2.1 | `int_2_1_exposure_then_risk_check` | 仓位暴露计算 → 开仓风控检查 |
| INT-2.2 | `int_2_2_exposure_limit_reached` | 暴露达上限 → 开仓被拒 |
| INT-3.1 | `int_3_1_pnl_then_drawdown` | 未实现盈亏 → 回撤计算链路 |
| INT-4.1 | `int_4_1_liquidation_then_drawdown` | 强平预警 → 回撤检查链路 |
| INT-4.2 | `int_4_2_risk_checker_record_and_check` | 记录亏损 → should_reduce |
| INT-5.1 | `int_5_1_tracker_record_then_snapshot` | 记录交易 → 快照 equity |
| INT-6.1 | `int_6_1_funding_rate_severity_threshold` | 费率 *2 倍 → critical |
| INT-6.2 | `int_6_2_drawdown_boundary_values` | 回撤各级别精确验证 |

---

## 3. 代码覆盖率

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/risk_tests.rs` | risk.rs | 18 |
| `src/tracker_tests.rs` | tracker.rs | 13 |
| `tests/integration_tests.rs` | 跨模块链路 | 10 |
| **合计** | | **41** |
