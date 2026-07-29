# virs-types 测试用例文档

> 生成日期: 2026-07-01
> Crate: `crates/libs/virs-types`
> 状态: **34 个测试全部通过** (32 单元 + 2 集成)

---

## 单元测试用例

### enums_tests.rs — 枚举方法 (2)

| ID | 测试函数 | 描述 |
|----|---------|------|
| E11.1 | `e11_1_open_is_open` | PositionStatus::Open → true, Closed → false |
| E14.1 | `e14_1_running_is_running` | EngineState::Running → true, Stopped → false |

### market_tests.rs — 市场类型方法 (2)

| ID | 测试函数 | 描述 |
|----|---------|------|
| M1.1 | `m1_1_normal_total` | free=100, used=50 → total=150 |
| M1.2 | `m1_2_zero_total` | free=0, used=0 → total=0 |

### position_tests.rs — Position/apply_fill 方法 (16)

| ID | 测试函数 | 描述 |
|----|---------|------|
| P1.1 | `p1_1_open_is_open` | Position(status=Open) → is_open=true |
| P6.1 | `p6_1_long_pnl` | Long, entry=50000, current=51000 → +1000 |
| P6.2 | `p6_2_short_pnl` | Short, entry=50000, current=49000 → +1000 |
| P2.1 | `p2_1_open_first_fill` | 开仓首次 fill：status=Open, qty=1, entry=100, rp=0 |
| P2.2 | `p2_2_open_add_to_existing_weighted_avg` | 加仓加权平均：(100*1+120*1)/2=110 |
| P2.3 | `p2_3_open_zero_fill_price_does_not_update_entry` | fill_price=0 时 quantity 更新但 entry_price 不变 |
| P2.4 | `p2_4_close_partial_entry_unchanged` | 部分平仓：entry 不变, qty 减半, rp=50 |
| P2.5 | `p2_5_close_full_returns_closed` | 全平：status=Closed, qty=0, rp=100 |
| P2.6 | `p2_6_close_with_zero_fill_price_still_updates` | REG-1 回归：平仓 fill_price=0 仓位仍更新 |
| P2.7 | `p2_7_open_after_partial_close_marginal_cost` | 平仓后重开边际成本法：(100*1+120*1)/2=110 |
| P2.8 | `p2_8_full_replay_sequence_matches_runtime` | 完整回放序列：开→平→开跨代际 rp 保留 |
| P3.1 | `p3_1_new_for_replay_initial_state` | new_for_replay 初始状态：Opening, qty/entry/rp=0 |
| P3.2 | `p3_2_new_for_replay_then_apply_fill_matches_runtime` | new_for_replay + apply_fill 与 runtime 一致 |
| P4.1 | `p4_1_uuid_deterministic_same_inputs` | 相同输入生成相同 UUID |
| P4.2 | `p4_2_uuid_differs_by_side` | 不同 side 生成不同 UUID |
| P4.3 | `p4_3_uuid_differs_by_symbol` | 不同 symbol 生成不同 UUID |

### serde_tests.rs — 序列化/反序列化 (7)

| ID | 测试函数 | 描述 |
|----|---------|------|
| S1.1 | `s1_1_side_roundtrip` | Side serde 往返 |
| S1.2 | `s1_2_order_status_roundtrip` | OrderStatus serde 往返 |
| S1.3 | `s1_3_market_type_roundtrip` | MarketType serde 往返 |
| S1.4 | `s1_4_strategy_status_roundtrip` | StrategyStatus serde 往返 |
| S2.1 | `s2_1_ticker_roundtrip` | Ticker serde 往返 |
| S2.2 | `s2_2_balance_roundtrip` | Balance serde 往返 |
| S2.3 | `s2_3_exchange_position_roundtrip` | ExchangePosition serde 往返 |

### client_order_id_tests.rs — 客户端订单 ID 生成 (5)

| ID | 测试函数 | 描述 |
|----|---------|------|
| C1.1 | `test_auto_open_long` | format_auto_open(long) → 前缀 `AOL__`, 长度 27 |
| C1.2 | `test_auto_open_short` | format_auto_open(short) → 前缀 `AOS__`, 长度 27 |
| C2.1 | `test_auto_close_long` | format_auto_close(long) → 前缀 `ACL__`, 长度 27 |
| C2.2 | `test_auto_close_short` | format_auto_close(short) → 前缀 `ACS__`, 长度 27 |
| C3.1 | `test_uniqueness` | 同一 bot_id 生成 1000 个 ID 全部唯一 |

---

## 集成测试用例

### integration_tests.rs (2)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_long_position_pnl_chain` | Long Position → unrealized_pnl_at 链路 |
| INT-1.2 | `int_1_2_short_position_pnl_chain` | Short Position → unrealized_pnl_at 链路 |

---

## 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/enums_tests.rs` | enums.rs | 2 |
| `src/market_tests.rs` | market.rs | 2 |
| `src/position_tests.rs` | position.rs | 16 |
| `src/serde_tests.rs` | 全部结构体 serde | 7 |
| `src/client_order_id_tests.rs` | client_order_id.rs | 5 |
| `tests/integration_tests.rs` | 跨模块计算链 + serde 联合 | 2 |
| **合计** | | **34** |

### 死代码清理记录

本次清理删除 29 个仅被测试引用的方法，测试数从 80 降至 34。

| 已删除方法 | 位置 | 业务调用方 |
|-----------|------|-----------|
| `Side::as_str` | enums.rs | 无（virs-ccxt 用 match 而非 as_str） |
| `Side::is_opening_for` | enums.rs | 无 |
| `Side::is_closing_for` | enums.rs | 无 |
| `PositionSide::as_str` | enums.rs | 无 |
| `PositionSide::is_long` | enums.rs | 无（调用者 ExchangePosition::is_long、Position::is_long 均已删除） |
| `PositionSide::is_short` | enums.rs | 无（同上） |
| `OrderStatus::is_open` | enums.rs | 无（调用者 PositionOrder::is_open 已删除） |
| `OrderStatus::is_canceled` | enums.rs | 无（调用者 PositionOrder::is_canceled 已删除） |
| `PositionStatus::is_closed` | enums.rs | 无（调用者 Position::is_closed 已删除） |
| `PositionStatus::is_empty` | enums.rs | 无（调用者 Position::is_empty 已删除） |
| `EngineState::is_stopped` | enums.rs | 无 |
| `StrategyStatus::is_running` | enums.rs | 无（virs-models 有同名独立方法） |
| `StrategyStatus::is_stopped` | enums.rs | 无（同上） |
| `ExchangePosition::is_long` | market.rs | 无 |
| `ExchangePosition::is_short` | market.rs | 无 |
| `ExchangePosition::pnl_pct_at` | market.rs | 无 |
| `AutoMarketType::is_perpetual` | auto_port.rs | 无 |
| `BotPositionSide::as_str` | bot.rs | 无 |
| `BotPositionSide::is_long` | bot.rs | 无 |
| `BotPositionSide::is_short` | bot.rs | 无 |
| `Position::is_closed` | position.rs | 无 |
| `Position::is_empty` | position.rs | 无 |
| `Position::is_long` | position.rs | 无 |
| `Position::is_short` | position.rs | 无 |
| `Position::pnl_pct_at` | position.rs | 无 |
| `PositionOrder::is_filled` | position.rs | 无 |
| `PositionOrder::is_open` | position.rs | 无 |
| `PositionOrder::is_canceled` | position.rs | 无 |
| `PositionOrder::fill_rate` | position.rs | 无 |

### 存活方法验证

以下方法有外部 crate 非测试代码调用，予以保留：

| 方法 | 业务调用位置 |
|------|-------------|
| `OrderStatus::is_filled` | `virs-position/src/engine.rs` |
| `PositionStatus::is_open` | `virs-types/src/position.rs` (Position::is_open 实现) |
| `EngineState::is_running` | `virs-position/src/engine.rs` |
| `Balance::compute_total` | `virs-exchange/src/paper.rs` |
| `ExchangePosition::unrealized_pnl_at` | `virs-exchange/src/paper.rs` |
| `Position::is_open` | `virs-bot/src/auto/worker.rs`, `virs-position/src/engine.rs` |
| `Position::unrealized_pnl_at` | `virs-bot/src/auto/strategy.rs` |
| `RiskConfig::validate` | `virs-position/src/risk.rs` |
| `OrderSide::as_str` | `virs-bot/src/auto/worker.rs` |
