# virs-types 测试用例文档

> 生成日期: 2026-07-01
> Crate: `crates/libs/virs-types`
> 状态: **34 个测试全部通过** (27 单元 + 7 集成)

---

## 单元测试用例

### enums_tests.rs — 枚举方法 (3)

| ID | 测试函数 | 描述 |
|----|---------|------|
| E7.1 | `e7_1_filled_is_filled` | OrderStatus::Filled → true, Open → false |
| E11.1 | `e11_1_open_is_open` | PositionStatus::Open → true, Closed → false |
| E14.1 | `e14_1_running_is_running` | EngineState::Running → true, Stopped → false |

### market_tests.rs — 市场类型方法 (5)

| ID | 测试函数 | 描述 |
|----|---------|------|
| M1.1 | `m1_1_normal_total` | free=100, used=50 → total=150 |
| M1.2 | `m1_2_zero_total` | free=0, used=0 → total=0 |
| M10.1 | `m10_1_long_profit` | Long, entry=50000, current=51000 → +1000 |
| M10.2 | `m10_2_short_profit` | Short, entry=50000, current=49000 → +1000 |
| M10.3 | `m10_3_long_loss` | Long, entry=50000, current=49000 → -1000 |

### auto_port_tests.rs — AutoMarketType 方法 (1)

| ID | 测试函数 | 描述 |
|----|---------|------|
| AM2.1 | `am2_1_spot_is_spot` | Spot → true, Perpetual → false |

### position_tests.rs — Position/RiskConfig 方法 (7)

| ID | 测试函数 | 描述 |
|----|---------|------|
| P1.1 | `p1_1_open_is_open` | Position(status=Open) → is_open=true |
| P6.1 | `p6_1_long_pnl` | Long, entry=50000, current=51000 → +1000 |
| P6.2 | `p6_2_short_pnl` | Short, entry=50000, current=49000 → +1000 |
| P12.1 | `p12_1_default_valid` | RiskConfig::default() → Ok |
| P12.2 | `p12_2_zero_leverage` | max_leverage=0 → Err |
| P12.3 | `p12_3_negative_drawdown` | max_drawdown_pct<0 → Err |
| P12.4 | `p12_4_negative_position_pct` | max_position_per_symbol_pct<0 → Err |

### serde_tests.rs — 序列化/反序列化 (11)

| ID | 测试函数 | 描述 |
|----|---------|------|
| S1.1 | `s1_1_side_roundtrip` | Side serde 往返 |
| S1.2 | `s1_2_order_status_roundtrip` | OrderStatus serde 往返 |
| S1.3 | `s1_3_market_type_roundtrip` | MarketType serde 往返 |
| S1.4 | `s1_4_strategy_status_roundtrip` | StrategyStatus serde 往返 |
| S2.1 | `s2_1_ticker_roundtrip` | Ticker serde 往返 |
| S2.2 | `s2_2_balance_roundtrip` | Balance serde 往返 |
| S2.3 | `s2_3_exchange_position_roundtrip` | ExchangePosition serde 往返 |
| S2.5 | `s2_5_risk_config_default_roundtrip` | RiskConfig serde 往返 |
| S4.1 | `s4_1_perpetual` | AutoMarketType::from_str_lossy("perpetual") → Perpetual |
| S4.2 | `s4_2_spot` | AutoMarketType::from_str_lossy("spot") → Spot |
| S4.3 | `s4_3_unknown_defaults_to_perpetual` | 未知值 → Perpetual (默认) |

---

## 集成测试用例

### integration_tests.rs (7)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_long_position_pnl_chain` | Long Position → unrealized_pnl_at 链路 |
| INT-1.2 | `int_1_2_short_position_pnl_chain` | Short Position → unrealized_pnl_at 链路 |
| INT-3.1 | `int_3_1_exchange_position_pnl_chain` | ExchangePosition → unrealized_pnl_at 链路 |
| INT-6.1 | `int_6_1_default_config_valid` | RiskConfig::default() → validate Ok |
| INT-6.2 | `int_6_2_invalid_config` | max_leverage=0 → validate Err |
| INT-8.1 | `int_8_1_exchange_position_serde_then_pnl` | serde 往返后 unrealized_pnl_at 一致 |
| INT-8.3 | `int_8_3_auto_market_type_from_str` | from_str_lossy("spot").is_spot() → true |

---

## 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/enums_tests.rs` | enums.rs (3 个存活方法) | 3 |
| `src/market_tests.rs` | market.rs (2 个存活方法) | 5 |
| `src/auto_port_tests.rs` | auto_port.rs (1 个存活方法) | 1 |
| `src/position_tests.rs` | position.rs (2 个存活方法 + RiskConfig) | 7 |
| `src/serde_tests.rs` | 全部结构体 serde | 11 |
| `tests/integration_tests.rs` | 跨模块计算链 + serde 联合 | 7 |
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

以下 11 个方法有外部 crate 非测试代码调用，予以保留：

| 方法 | 业务调用位置 |
|------|-------------|
| `OrderStatus::is_filled` | `virs-position/src/engine.rs` |
| `PositionStatus::is_open` | `virs-types/src/position.rs` (Position::is_open 实现) |
| `EngineState::is_running` | `virs-position/src/engine.rs` |
| `Balance::compute_total` | `virs-ccxt/src/adapter/binance/api.rs`, `virs-exchange/src/paper.rs` |
| `ExchangePosition::unrealized_pnl_at` | `virs-exchange/src/paper.rs` |
| `AutoMarketType::as_str` | `virs-bot/src/auto/worker.rs` |
| `AutoMarketType::is_spot` | `virs-bot/src/auto/worker.rs` |
| `AutoMarketType::from_str_lossy` | `virs-app/src/adapters/auto_store.rs` |
| `Position::is_open` | `virs-bot/src/auto/worker.rs`, `virs-position/src/engine.rs` |
| `Position::unrealized_pnl_at` | `virs-bot/src/auto/strategy.rs` |
| `RiskConfig::validate` | `virs-position/src/risk.rs` |
| `OrderSide::as_str` | `virs-bot/src/grid/worker.rs` |
