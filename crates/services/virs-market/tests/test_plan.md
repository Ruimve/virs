# virs-market 测试用例文档

> Crate: `crates/services/virs-market`
> 状态: **38 个测试全部通过** (30 单元 + 8 集成)

---

## 1. 单元测试用例

### 1.1 types_tests.rs — 类型与工具函数 (11)

| ID | 测试函数 | 描述 |
|----|---------|------|
| T1.1 | `t1_1_timeframe_all_count` | all() → 6 个变体 |
| T1.2 | `t1_2_timeframe_as_str` | 每个变体 → 正确字符串 |
| T1.3 | `t1_3_timeframe_ms` | 各变体 ms 值正确 |
| T1.5 | `t1_5_timeframe_default_limit` | 所有 → 1000 |
| T1.7 | `t1_7_timeframe_display` | format!("{}", M1) → "1m" |
| T2.1 | `t2_1_subscription_key_basic` | ("binance", "BTC/USDT") → "binance:BTC/USDT" |
| T2.2 | `t2_2_subscription_key_case` | exchange 小写, symbol 大写 |
| T3.1 | `t3_1_align_open_time_m1` | 任意时间 → 对齐到分钟 |
| T3.2 | `t3_2_align_open_time_h1` | 任意时间 → 对齐到小时 |
| T3.3 | `t3_3_align_open_time_d1` | 任意时间 → 对齐到天 |
| T3.4 | `t3_4_align_open_time_exact` | 已对齐 → 不变 |

### 1.2 aggregator_tests.rs — K线聚合 (12)

| ID | 测试函数 | 描述 |
|----|---------|------|
| A1.1 | `a1_1_candle_from_1m_basic` | 基础转换: open_time 对齐, OHLCV 复制 |
| A1.2 | `a1_2_candle_from_1m_align` | open_time 不对齐 → 对齐到 timeframe |
| A1.3 | `a1_3_candle_from_1m_closed_false` | 新建蜡烛 closed=false |
| A2.1 | `a2_1_is_last_1m_in_group_m5` | 第5根1m → M5 组末尾 (true) |
| A2.2 | `a2_2_is_last_1m_not_last` | 第3根1m → M5 组非末尾 (false) |
| A2.3 | `a2_3_is_last_1m_in_group_h1` | 第60根1m → H1 组末尾 (true) |
| A2.4 | `a2_4_is_last_1m_exact_boundary` | open_time 恰好为组边界 |
| A3.1 | `a3_1_aggregate_empty` | 空输入 → 空输出 |
| A3.2 | `a3_2_aggregate_single_candle` | 1根1m → 1根高周期 |
| A3.3 | `a3_3_aggregate_m5_full` | 5根1m → 1根 M5, OHLC 正确 |
| A3.4 | `a3_4_aggregate_m5_partial` | 3根1m → 1根未关闭 M5 |
| A3.5 | `a3_5_aggregate_multi_group` | 7根1m → 2根 (5+2), 第二根未关闭 |

### 1.3 cache_tests.rs — 符号缓存 (7)

| ID | 测试函数 | 描述 |
|----|---------|------|
| C1.1 | `c1_1_update_and_get` | 更新蜡烛 → 读取一致 |
| C1.2 | `c1_2_update_same_open_time` | 同 open_time → 覆盖 |
| C1.3 | `c1_3_update_old_candle` | 更新旧蜡烛 → 插入正确位置 |
| C1.4 | `c1_4_max_size_eviction` | 超出容量 → 淘汰最旧 |
| C2.1 | `c2_1_close_candle` | 标记关闭 → closed=true |
| C2.2 | `c2_2_last_closed_1m` | 获取最后关闭的1m蜡烛 |
| C3.1 | `c3_1_replace_timeframe` | 替换整个时间周期数据 |

---

## 2. 集成测试用例

### integration_tests.rs (8)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.2 | `int_1_2_align_then_aggregate` | align_open_time → aggregate 链路 |
| INT-2.1 | `int_2_1_candle_from_1m_then_aggregate` | candle_from_1m → aggregate_1m 一致性 |
| INT-2.2 | `int_2_2_aggregate_then_cache_update` | aggregate → cache.update_candle 链路 |
| INT-3.1 | `int_3_1_subscription_key_then_check` | subscription_key → 一致性验证 |
| INT-3.2 | `int_3_2_align_multi_timeframe` | 同一 open_time 不同周期对齐 |
| INT-5.1 | `int_5_1_gap_detection_logic` | 模拟缺口: 2根1m间隔 → 聚合验证 |
| INT-5.2 | `int_5_2_aggregate_full_day_to_d1` | 1440根1m → 1根D1, OHLCV 正确 |
| INT-6.1 | `int_6_1_timeframe_str_to_ms` | "1m"→60000, "1h"→3600000 等 |

---

## 3. 代码覆盖率

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/types_tests.rs` | types.rs | 11 |
| `src/aggregator_tests.rs` | aggregator.rs | 12 |
| `src/cache_tests.rs` | cache.rs | 7 |
| `tests/integration_tests.rs` | 跨模块链路 | 8 |
| **合计** | | **38** |
