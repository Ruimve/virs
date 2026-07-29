# virs-models 测试用例文档

> 生成日期: 2026-07-01
> Crate: `crates/libs/virs-models`
> 状态: **17 个测试全部通过** (12 单元 + 2 serde + 3 集成)

---

## 单元测试用例

### auto_tests.rs — AutoBot 方法 (12)

| ID | 测试函数 | 描述 |
|----|---------|------|
| A1.1 | `a1_1_seventy_percent` | win=7, total=10 → 70.0 |
| A1.2 | `a1_2_zero_wins` | win=0, total=10 → 0.0 |
| A1.3 | `a1_3_zero_trades_division_protection` | total=0 → 0.0 (除零保护) |
| A2.1 | `a2_1_thirty_percent` | loss=3, total=10 → 30.0 |
| A2.2 | `a2_2_zero_trades_division_protection` | total=0 → 0.0 (除零保护) |
| A3.1 | `a3_1_positive_return` | total_pnl=1000, capital=10000 → 10.0 |
| A3.2 | `a3_2_negative_return` | total_pnl=-500, capital=10000 → -5.0 |
| A3.3 | `a3_3_zero_capital_division_protection` | capital=0 → 0.0 (除零保护) |
| A4.1 | `a4_1_running_status` | status="running" → is_running()=true |
| A4.2 | `a4_2_stopped_status` | status="stopped" → is_running()=false |
| A5.1 | `a5_1_stopped_status` | status="stopped" → is_stopped()=true |
| A5.2 | `a5_2_running_status` | status="running" → is_stopped()=false |

### serde_tests.rs — 序列化/反序列化 (2)

| ID | 测试函数 | 描述 |
|----|---------|------|
| S1.1 | `s1_1_order_roundtrip` | Order serde 往返 |
| S4.1 | `s4_1_auto_bot_roundtrip` | AutoBot serde 往返 |

---

## 集成测试用例

### integration_tests.rs (3)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-3.1 | `int_3_1_win_plus_loss_equals_100` | win_rate + loss_rate = 100 + total_return_pct |
| INT-3.2 | `int_3_2_negative_return` | 负收益率 + is_stopped + win_rate |
| INT-5.2 | `int_5_2_auto_bot_serde_then_win_rate` | AutoBot serde 往返后 win_rate 一致 |

---

## 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/auto_tests.rs` | auto.rs (AutoBot 5 方法) | 12 |
| `src/serde_tests.rs` | 全部结构体 serde | 2 |
| `tests/integration_tests.rs` | 跨模块计算链 + serde 联合 | 3 |
| **合计** | | **17** |

### 死代码清理记录

无死代码。全部 5 个公开方法均在 `virs-api` 业务代码中被调用：

| 方法 | 业务调用位置 |
|------|-------------|
| `AutoBot::win_rate` | `virs-api/src/handlers/auto_trade.rs` |
| `AutoBot::loss_rate` | `virs-api/src/handlers/auto_trade.rs` |
| `AutoBot::total_return_pct` | `virs-api/src/handlers/auto_trade.rs` |
| `AutoBot::is_running` | `virs-api/src/handlers/auto_trade.rs` |
| `AutoBot::is_stopped` | `virs-api/src/handlers/auto_trade.rs` |
