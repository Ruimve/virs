# virs-models 测试用例文档

> 生成日期: 2026-07-01
> Crate: `crates/libs/virs-models`
> 状态: **37 个测试全部通过** (31 单元 + 6 集成)

---

## 单元测试用例

### grid_tests.rs — GridBot 方法 (8)

| ID | 测试函数 | 描述 |
|----|---------|------|
| G1.1 | `g1_1_normal_spacing` | upper=50000, lower=40000, grid_count=10 → 1000.0 |
| G1.2 | `g1_2_zero_range` | upper=lower → 0.0 |
| G1.3 | `g1_3_zero_grid_count` | grid_count=0 → 0.0 (除零保护) |
| G3.1 | `g3_1_running_status` | status=Running → true |
| G3.2 | `g3_2_stopped_status` | status=Stopped → false |
| G4.1 | `g4_1_stopped_status` | status=Stopped → true |
| G4.2 | `g4_2_running_status` | status=Running → false |
| G5.3 | `g5_3_zero_capital_division_protection` | initial_capital=0 → 0.0 (除零保护) |

> 另含 G5.1 (正收益率) 和 G5.2 (零收益率)，共 10 个测试。

### auto_tests.rs — AutoBot 方法 (10)

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
| A4.1 | `a4_1_running_status` | status="running" → true |
| A4.2 | `a4_2_stopped_status` | status="stopped" → false |

> 另含 A5.1/A5.2 (is_stopped 正反判断)，共 12 个测试。

### serde_tests.rs — 序列化/反序列化 (9)

| ID | 测试函数 | 描述 |
|----|---------|------|
| S1.1 | `s1_1_order_roundtrip` | Order serde 往返 |
| S2.1 | `s2_1_user_roundtrip` | User serde 往返 |
| S2.2 | `s2_2_user_response_roundtrip` | UserResponse serde 往返 |
| S2.3 | `s2_3_login_request_deserialize` | LoginRequest 反序列化 |
| S2.4 | `s2_4_create_user_request_with_role_none` | CreateUserRequest (role=None) 反序列化 |
| S3.1 | `s3_1_grid_bot_roundtrip` | GridBot serde 往返 |
| S3.2 | `s3_2_grid_trade_roundtrip` | GridTrade serde 往返 |
| S4.1 | `s4_1_auto_bot_roundtrip` | AutoBot serde 往返 |
| S4.2 | `s4_2_auto_trade_roundtrip` | AutoTrade serde 往返 |

---

## 集成测试用例

### integration_tests.rs (6)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-2.1 | `int_2_1_spacing_and_return_pct` | GridBot 多方法联合验证 (spacing + is_running + total_return_pct) |
| INT-2.2 | `int_2_2_invalid_config_negative_spacing` | upper<lower → 负 spacing + is_stopped |
| INT-3.1 | `int_3_1_win_plus_loss_equals_100` | win_rate + loss_rate = 100 + total_return_pct |
| INT-3.2 | `int_3_2_negative_return` | 负收益率 + is_stopped + win_rate |
| INT-5.1 | `int_5_1_grid_bot_serde_then_methods` | GridBot serde 往返后 grid_spacing 一致 |
| INT-5.2 | `int_5_2_auto_bot_serde_then_win_rate` | AutoBot serde 往返后 win_rate 一致 |

---

## 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/grid_tests.rs` | grid.rs (GridBot 4 方法) | 10 |
| `src/auto_tests.rs` | auto.rs (AutoBot 5 方法) | 12 |
| `src/serde_tests.rs` | 全部结构体 serde | 9 |
| `tests/integration_tests.rs` | 跨模块计算链 + serde 联合 | 6 |
| **合计** | | **37** |

### 死代码清理记录

无死代码。全部 9 个公开方法均在 `virs-api` 业务代码中被调用：

| 方法 | 业务调用位置 |
|------|-------------|
| `GridBot::grid_spacing` | `virs-api/src/handlers/grid.rs` |
| `GridBot::is_running` | `virs-api/src/handlers/grid.rs` |
| `GridBot::is_stopped` | `virs-api/src/handlers/grid.rs` |
| `GridBot::total_return_pct` | `virs-api/src/handlers/grid.rs` |
| `AutoBot::win_rate` | `virs-api/src/handlers/auto_trade.rs` |
| `AutoBot::loss_rate` | `virs-api/src/handlers/auto_trade.rs` |
| `AutoBot::total_return_pct` | `virs-api/src/handlers/auto_trade.rs` |
| `AutoBot::is_running` | `virs-api/src/handlers/auto_trade.rs` |
| `AutoBot::is_stopped` | `virs-api/src/handlers/auto_trade.rs` |
