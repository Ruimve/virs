# Kline Engine Test Report

> Generated: 2026-04-26 | Total: 93 tests | Status: ✅ ALL PASSED

---

## Overview

| Module | Tests | Type | Mock |
|--------|-------|------|------|
| types.rs | 19 | Unit | None |
| aggregator.rs | 21 | Unit | None |
| cache.rs | 22 | Unit | None |
| gap.rs | 11 | Integration | MockKlineSource |
| mod.rs | 20 | Integration | MockKlineSource |
| **Total** | **93** | | |

---

## 1. types.rs — Pure Data Layer (19 tests)

### 1.1 Timeframe Enum

#### TC-T-001: test_timeframe_ms
- **Scenario**: 验证每个 Timeframe 变体的毫秒数计算是否正确
- **Input**: `Timeframe::M1`, `M5`, `M15`, `H1`, `H4`, `D1`
- **Expected**: `M1=60000`, `M5=300000`, `M15=900000`, `H1=3600000`, `H4=14400000`, `D1=86400000`
- **Rationale**: 毫秒数是聚合计算的基础，错误会导致 K 线时间对齐偏移

#### TC-T-002: test_timeframe_as_str
- **Scenario**: 验证 Timeframe 枚举到字符串的转换
- **Input**: 各 Timeframe 变体
- **Expected**: `M1→"1m"`, `M5→"5m"`, `M15→"15m"`, `H1→"1h"`, `H4→"4h"`, `D1→"1d"`
- **Rationale**: 字符串表示用于 API 参数和交易所请求，必须精确匹配

#### TC-T-003: test_timeframe_from_str_lossy
- **Scenario**: 验证字符串到 Timeframe 的容错解析
- **Input**: `"1m"`, `"5m"`, `"15m"`, `"1h"`, `"4h"`, `"1d"`, `"1D"`, `"2h"`, `""`
- **Expected**: 合法字符串返回对应变体；`"1D"` 大写也映射到 `D1`；非法输入返回 `None`
- **Rationale**: 前端和 API 传入的字符串可能大小写不一致，需容错处理

#### TC-T-004: test_timeframe_minutes
- **Scenario**: 验证每个 Timeframe 的分钟数
- **Input**: 各 Timeframe 变体
- **Expected**: `M1=1`, `M5=5`, `M15=15`, `H1=60`, `H4=240`, `D1=1440`
- **Rationale**: 分钟数用于缺口检测和回测范围计算

#### TC-T-005: test_timeframe_default_limit
- **Scenario**: 验证每个 Timeframe 的默认 K 线数量限制
- **Input**: 各 Timeframe 变体
- **Expected**: `M1=2000`, `M5=1000`, `M15=1000`, `H1=1000`, `H4=1000`, `D1=1000`
- **Rationale**: 限制值决定缓存容量和初始加载数据量，M1 需要更多以支持高周期聚合

#### TC-T-006: test_timeframe_all
- **Scenario**: 验证 `Timeframe::all()` 返回全部 6 个变体
- **Input**: `Timeframe::all()`
- **Expected**: 长度 6，包含 M1 和 D1
- **Rationale**: `all()` 用于遍历所有周期进行初始化和聚合

#### TC-T-007: test_timeframe_display
- **Scenario**: 验证 Display trait 实现
- **Input**: `format!("{}", Timeframe::M1)`, `H1`, `D1`
- **Expected**: `"1m"`, `"1h"`, `"1d"`
- **Rationale**: Display 用于日志和错误信息

#### TC-T-008: test_timeframe_serde
- **Scenario**: 验证 JSON 序列化/反序列化
- **Input**: `serde_json::to_string(&Timeframe::M1)`, `from_str::<Timeframe>("\"5m\"")`
- **Expected**: 序列化为 `"1m"`，反序列化 `"5m"` 为 `Timeframe::M5`
- **Rationale**: API 请求/响应的 JSON 传输依赖正确的 serde 实现

### 1.2 Candle Struct

#### TC-T-009: test_candle_merge
- **Scenario**: 验证 K 线合并逻辑 — 更新值覆盖基础值
- **Input**: base candle (high=110, low=95, close=105, volume=50, trades=100, closed=false) + update (high=115, low=90, close=108, volume=30, trades=50, closed=true)
- **Expected**: high=115(max), low=90(min), close=108(取更新), volume=80(累加), quote_volume=8000(累加), trades=150(累加), closed=true(取更新)
- **Rationale**: merge 是实时更新 K 线的核心操作，OHLCV 规则必须正确

#### TC-T-010: test_candle_merge_no_lower_high
- **Scenario**: 验证 merge 不会降低 high 值
- **Input**: base (high=120) + update (high=110)
- **Expected**: high 保持 120（取最大值）
- **Rationale**: 高价只能上升不能下降，这是 K 线数据的基本不变量

#### TC-T-011: test_candle_from_1m
- **Scenario**: 从 1m K 线创建高周期 K 线骨架
- **Input**: 1m candle (open_time=3600000, H1 timeframe)
- **Expected**: open_time=3600000(对齐), close_time=7199999(3600000+3600000-1), OHLCV 复制自 1m, closed=false(单根 1m 不能闭合高周期)
- **Rationale**: 首次收到 1m 数据时创建高周期 K 线的初始状态

#### TC-T-012: test_candle_from_1m_alignment
- **Scenario**: 验证 from_1m 自动对齐 open_time 到周期边界
- **Input**: 1m candle (open_time=3630000, H1 timeframe)
- **Expected**: open_time=3600000（向下对齐到 H1 边界）
- **Rationale**: 1m K 线的 open_time 可能不在高周期边界上，必须对齐

#### TC-T-013: test_candle_is_closed
- **Scenario**: 验证 is_closed() 方法
- **Input**: closed=true 和 closed=false 的 Candle
- **Expected**: 分别返回 true 和 false
- **Rationale**: 闭合状态影响缓存更新策略和事件广播类型

### 1.3 Utility Functions

#### TC-T-014: test_align_open_time
- **Scenario**: 验证时间戳对齐到周期边界
- **Input**:
  - `align_open_time(0, M5)` → 0
  - `align_open_time(60000, M5)` → 0（1分钟对齐到5分钟边界=0）
  - `align_open_time(300000, M5)` → 300000（恰好在边界）
  - `align_open_time(3600000, H1)` → 3600000（恰好在边界）
  - `align_open_time(3630000, H1)` → 3600000（向下对齐）
  - `align_open_time(86400000, D1)` → 86400000
  - `align_open_time(90000000, D1)` → 86400000
- **Expected**: 所有输入对齐到对应周期的整数倍
- **Rationale**: 时间对齐是 K 线聚合的基础，错误对齐导致数据错位

#### TC-T-015: test_subscription_key
- **Scenario**: 验证订阅键生成格式
- **Input**: `("Binance", "btcusdt")`, `("OKX", "BTC/USDT")`
- **Expected**: `"binance:BTCUSDT"`, `"okx:BTC/USDT"`（exchange 小写，symbol 大写）
- **Rationale**: 订阅键用于 DashMap 查找，格式必须一致

#### TC-T-016: test_binance_ws_symbol
- **Scenario**: 验证 Binance WebSocket symbol 格式转换
- **Input**: `"BTCUSDT"`, `"BTC/USDT"`, `"ETH/USDT"`
- **Expected**: `"btcusdt"`, `"btcusdt"`, `"ethusdt"`（去斜杠 + 小写）
- **Rationale**: Binance WS 使用小写无斜杠格式，转换错误导致订阅失败

#### TC-T-017: test_kline_event_type_serde
- **Scenario**: 验证 KlineEventType 的 JSON 序列化
- **Input**: `Update`, `Closed`, `Backfilled`
- **Expected**: `"Update"`, `"Closed"`, `"Backfilled"`
- **Rationale**: 事件类型通过 WebSocket 传输给前端，序列化必须正确

#### TC-T-018: test_backtest_range_limit
- **Scenario**: 验证回测范围限制配置
- **Input**: `BacktestRangeLimit::for_timeframe(M1)`, `for_timeframe(D1)`
- **Expected**: M1 max_days=7, recommended_days=3; D1 max_days=1825; estimated_1m_required > estimated_candles
- **Rationale**: 回测范围限制防止内存溢出，M1 需要大量 1m 数据聚合高周期

#### TC-T-019: test_kline_engine_config_default
- **Scenario**: 验证引擎默认配置
- **Input**: `KlineEngineConfig::default()`
- **Expected**: ws_base_url_spot="wss://stream.binance.com/ws", ws_base_url_perpetual="wss://fstream.binance.com/ws", backfill_on_start=true, event_channel_capacity=8192
- **Rationale**: 默认配置必须与 Binance API 文档一致

---

## 2. aggregator.rs — Stateless Aggregation (21 tests)

### 2.1 Batch Aggregation (aggregate_1m_to_timeframe)

#### TC-A-001: test_aggregate_5m_single_group
- **Scenario**: 5 根 1m K 线聚合为 1 根 5m K 线
- **Input**: 5 根 1m (open_time=0,60000,120000,180000,240000)，价格递增
- **Expected**: 1 根 5m，open=100(首根open), high=115(所有max), low=99(所有min), close=111(末根close), volume=56(累加), closed=true
- **Rationale**: 验证 OHLCV 聚合的基本正确性

#### TC-A-002: test_aggregate_two_5m_groups
- **Scenario**: 10 根 1m K 线聚合为 2 根 5m K 线
- **Input**: 10 根连续 1m (start_time=0)
- **Expected**: 2 根 5m，open_time 分别为 0 和 300000
- **Rationale**: 验证多组聚合和边界划分

#### TC-A-003: test_aggregate_empty
- **Scenario**: 空 1m 数据聚合
- **Input**: `[]`
- **Expected**: 空结果
- **Rationale**: 边界条件，空输入不应 panic

#### TC-A-004: test_aggregate_single_candle
- **Scenario**: 单根 1m K 线聚合为 5m
- **Input**: 1 根 1m (open_time=0)
- **Expected**: 1 根 5m，closed=false（不够 5 根无法闭合）
- **Rationale**: 不完整组应标记为未闭合

#### TC-A-005: test_aggregate_partial_group_unclosed
- **Scenario**: 3 根 1m 聚合为 5m（不完整组）
- **Input**: 3 根 1m (open_time=0,60000,120000)
- **Expected**: 1 根 5m，closed=false
- **Rationale**: 未满 5 根的组不能标记为闭合

#### TC-A-006: test_aggregate_15m
- **Scenario**: 15 根 1m 聚合为 1 根 15m
- **Input**: 15 根连续 1m
- **Expected**: 1 根 15m，close_time=899999，closed=true
- **Rationale**: 验证 15m 周期聚合

#### TC-A-007: test_aggregate_1h
- **Scenario**: 60 根 1m 聚合为 1 根 1h
- **Input**: 60 根连续 1m
- **Expected**: 1 根 1h，close_time=3599999，closed=true
- **Rationale**: 验证 1h 周期聚合

#### TC-A-008: test_aggregate_4h
- **Scenario**: 240 根 1m 聚合为 1 根 4h
- **Input**: 240 根连续 1m
- **Expected**: 1 根 4h，close_time=14399999，closed=true
- **Rationale**: 验证 4h 周期聚合

#### TC-A-009: test_aggregate_1d
- **Scenario**: 1440 根 1m 聚合为 1 根 1d
- **Input**: 1440 根连续 1m
- **Expected**: 1 根 1d，close_time=86399999，closed=true
- **Rationale**: 验证 1d 周期聚合

#### TC-A-010: test_aggregate_1d_two_days
- **Scenario**: 2880 根 1m 聚合为 2 根 1d
- **Input**: 2880 根连续 1m
- **Expected**: 2 根 1d，open_time 分别为 0 和 86400000
- **Rationale**: 验证跨日边界聚合

#### TC-A-011: test_aggregate_mixed_closed_unclosed
- **Scenario**: 组内包含未闭合 1m K 线
- **Input**: 4 根 1m，最后一根 closed=false
- **Expected**: 聚合结果 closed=false
- **Rationale**: 组内任一 K 线未闭合，整组标记为未闭合

#### TC-A-012: test_aggregate_high_low_tracking
- **Scenario**: 验证聚合过程中 high/low 的正确追踪
- **Input**: 5 根 1m，其中第 2 根 high=120（全局最高），第 3 根 low=90（全局最低）
- **Expected**: high=120, low=90
- **Rationale**: high/low 必须取组内极值，而非最后一根的值

#### TC-A-013: test_aggregate_volume_accumulation
- **Scenario**: 验证 volume 和 trades 的累加
- **Input**: 5 根 1m，volume 分别为 10,20,30,40,50，trades 均为 100
- **Expected**: volume=150, trades=500
- **Rationale**: 成交量和成交笔数必须累加，不能取最后一根

#### TC-A-014: test_aggregate_non_aligned_start
- **Scenario**: 1m 数据起始时间不在 5m 边界上
- **Input**: 8 根 1m (start_time=120000，即第 3 分钟)
- **Expected**: 结果不为空，首根 open_time=0（对齐到 5m 边界），open 取首根 1m 的 open
- **Rationale**: 即使 1m 数据不完整，聚合也应正确对齐到周期边界

#### TC-A-015: test_aggregate_gap_in_1m_data
- **Scenario**: 1m 数据中间有缺口
- **Input**: 前 3 根 (0-120000) + 后 3 根 (600000-720000)，中间缺 3 根
- **Expected**: 至少 2 组结果，open_time 分别为 0 和 600000
- **Rationale**: 聚合器应按 open_time 对齐分组，缺口自然形成不同组

### 2.2 Incremental Update (update_higher_timeframes)

#### TC-A-016: test_is_last_1m_in_group
- **Scenario**: 判断 1m K 线是否是 5m 组的最后一根
- **Input**: open_time=0(M5第1根), 240000(M5第5根), 300000(M5第1根)
- **Expected**: 0→false, 240000→true, 300000→false
- **Rationale**: 最后一根 K 线到达时标记高周期 K 线为闭合

#### TC-A-017: test_is_last_1m_in_group_1h
- **Scenario**: 判断 1m K 线是否是 1h 组的最后一根
- **Input**: open_time=1800000(30分钟，非最后), 3540000(59分钟，最后)
- **Expected**: 1800000→false, 3540000→true
- **Rationale**: H1 的最后一根是第 59 分钟

#### TC-A-018: test_is_last_1m_in_group_1d
- **Scenario**: 判断 1m K 线是否是 1d 组的最后一根
- **Input**: open_time=86340000(23:59)
- **Expected**: true
- **Rationale**: D1 的最后一根是 23:59

#### TC-A-019: test_update_higher_timeframes_first_candle
- **Scenario**: 首次收到 1m K 线，触发所有高周期创建
- **Input**: 1 根 1m (open_time=0, closed=false)，空 cache
- **Expected**: 返回 5 个更新 (M5/M15/H1/H4/D1)，所有 open_time=0，所有 closed=false
- **Rationale**: 首根 K 线应同时创建所有高周期 K 线骨架

#### TC-A-020: test_update_higher_timeframes_closing_5m
- **Scenario**: 第 5 根 1m 到达，5m K 线闭合
- **Input**: 先插入 4 根 1m (0-180000)，再插入第 5 根 (240000, closed=true)
- **Expected**: 返回的 M5 更新中 closed=true
- **Rationale**: 5m 组满 5 根 1m 后应标记为闭合

#### TC-A-021: test_update_higher_timeframes_new_period
- **Scenario**: 新 5m 周期开始，创建新 K 线
- **Input**: 先插入 1m (open_time=0)，再插入 1m (open_time=300000，新 5m 周期)
- **Expected**: M5 更新 open_time=300000, open=110.0（新 K 线）
- **Rationale**: 跨越周期边界时应创建新的高周期 K 线

---

## 3. cache.rs — In-Memory Ring Buffer (22 tests)

### 3.1 Basic Operations

#### TC-C-001: test_symbol_cache_new
- **Scenario**: 新建缓存，验证初始状态
- **Input**: `SymbolCache::new()`
- **Expected**: 所有 Timeframe 的 candle_count=0，is_empty=true
- **Rationale**: 确保初始化状态干净

#### TC-C-002: test_update_candle_append
- **Scenario**: 追加新 K 线（open_time 递增）
- **Input**: 2 根 K 线 (open_time=0, 60000)
- **Expected**: candle_count=2
- **Rationale**: 最常见的写入路径

#### TC-C-003: test_update_candle_same_open_time_overwrite
- **Scenario**: 相同 open_time 的 K 线覆盖更新
- **Input**: 先插入 (open_time=0, price=100, closed=false)，再插入 (open_time=0, price=110, closed=true)
- **Expected**: candle_count=1，open=110，closed=true
- **Rationale**: 实时更新场景，同一分钟的 K 线应覆盖而非追加

#### TC-C-004: test_update_candle_older_update_in_place
- **Scenario**: 更新历史 K 线（open_time 小于最新）
- **Input**: 先插入 (0, price=100)，再插入 (60000, price=110)，再插入 (0, price=105, closed=true)
- **Expected**: candle_count=2，第一根 open=105, closed=true
- **Rationale**: 回填数据可能更新历史 K 线

#### TC-C-005: test_close_candle
- **Scenario**: 标记 K 线为闭合
- **Input**: 插入 unclosed K 线，然后 close_candle
- **Expected**: closed 从 false 变为 true
- **Rationale**: WS 推送闭合事件时需标记

#### TC-C-006: test_close_candle_nonexistent
- **Scenario**: 关闭不存在的 K 线
- **Input**: `close_candle(M1, 999)` 在空缓存上
- **Expected**: 不 panic，candle_count=0
- **Rationale**: 边界条件，不应崩溃

### 3.2 Retrieval

#### TC-C-007: test_last_closed_1m
- **Scenario**: 获取最后一根闭合的 1m K 线
- **Input**: 空 cache → None；插入 (0, closed=true) + (60000, closed=false)
- **Expected**: 返回 open_time=0 的 K 线
- **Rationale**: 缺口检测依赖 last_closed_1m 计算缺失范围

#### TC-C-008: test_last_1m
- **Scenario**: 获取最后一根 1m K 线（无论是否闭合）
- **Input**: 空 cache → None；插入 (0, closed=true) + (60000, closed=false)
- **Expected**: 返回 open_time=60000 的 K 线
- **Rationale**: 用于判断当前进度

### 3.3 Bulk Operations

#### TC-C-009: test_replace_timeframe
- **Scenario**: 整体替换某周期数据
- **Input**: 先插入 1 根，再 replace 为 10 根
- **Expected**: candle_count=10
- **Rationale**: 初始加载时整体替换缓存

#### TC-C-010: test_replace_timeframe_truncation
- **Scenario**: 替换数据超过容量限制时截断
- **Input**: replace (limit+100) 根 K 线
- **Expected**: candle_count=limit，首根 open_time=100*60000（丢弃最旧的 100 根）
- **Rationale**: 缓存容量限制必须严格执行

#### TC-C-011: test_backfill_timeframe_empty_cache
- **Scenario**: 空缓存回填
- **Input**: 空 cache + 5 根 K 线
- **Expected**: candle_count=5
- **Rationale**: 初始加载的常见路径

#### TC-C-012: test_backfill_timeframe_append_new
- **Scenario**: 回填数据追加到现有数据之后
- **Input**: 现有 2 根 (0, 60000) + 回填 3 根 (120000, 180000, 240000)
- **Expected**: candle_count=5
- **Rationale**: 补全缺口数据

#### TC-C-013: test_backfill_overwrite_unclosed_with_closed
- **Scenario**: 回填的 closed K 线覆盖 unclosed K 线
- **Input**: 现有 (0, price=100, closed=false) + 回填 (0, price=105, closed=true)
- **Expected**: open=105, closed=true
- **Rationale**: 交易所历史数据（已闭合）应覆盖本地未闭合数据

#### TC-C-014: test_backfill_no_overwrite_closed_with_closed
- **Scenario**: 回填的 closed K 线不覆盖已有的 closed K 线
- **Input**: 现有 (0, price=100, closed=true) + 回填 (0, price=105, closed=true)
- **Expected**: open=100（保持原值）
- **Rationale**: 已闭合的 K 线是确定的，不应被覆盖

#### TC-C-015: test_backfill_no_overwrite_closed_with_unclosed
- **Scenario**: 回填的 unclosed K 线不覆盖已有的 closed K 线
- **Input**: 现有 (0, price=100, closed=true) + 回填 (0, price=105, closed=false)
- **Expected**: open=100, closed=true
- **Rationale**: 已闭合 K 线优先级高于未闭合

### 3.4 Capacity & Edge Cases

#### TC-C-016: test_capacity_enforcement_on_update
- **Scenario**: 通过 update_candle 超过容量限制
- **Input**: 插入 (limit+50) 根 K 线
- **Expected**: candle_count=limit
- **Rationale**: 所有写入路径都必须执行容量限制

#### TC-C-017: test_get_all_timeframes
- **Scenario**: 一次获取所有周期数据
- **Input**: M1 插入 1 根，M5 插入 1 根
- **Expected**: m1.len=1, m5.len=1, m15.len=0
- **Rationale**: 前端一次性获取所有周期数据

#### TC-C-018: test_get_klines_nonexistent_timeframe
- **Scenario**: 查询未写入数据的周期
- **Input**: 空 cache，查询 M1
- **Expected**: 空 Vec
- **Rationale**: 未初始化的周期应返回空而非报错

#### TC-C-019: test_is_empty_after_data
- **Scenario**: 插入数据后 is_empty 应为 false
- **Input**: 插入 1 根 M1 K 线
- **Expected**: is_empty=false
- **Rationale**: is_empty 用于判断是否需要初始加载

#### TC-C-020: test_multiple_timeframes_independent
- **Scenario**: 不同周期的数据互不影响
- **Input**: M1 插入 2 根，M5 插入 1 根
- **Expected**: M1 count=2, M5 count=1, H1 count=0
- **Rationale**: 各周期缓存独立存储

#### TC-C-021: test_backfill_truncation
- **Scenario**: 回填数据超过容量限制时截断
- **Input**: backfill (limit+100) 根 K 线
- **Expected**: candle_count=limit
- **Rationale**: 回填路径也必须执行容量限制

#### TC-C-022: test_timeframe_buffer_push_or_update_order_preserved
- **Scenario**: 验证 K 线时间顺序保持
- **Input**: 依次插入 (0, price=100), (60000, price=110), (120000, price=120)
- **Expected**: klines[0].open_time=0, klines[1].open_time=60000, klines[2].open_time=120000
- **Rationale**: K 线必须按时间排序，乱序会导致图表显示错误

---

## 4. gap.rs — Gap Detection & Initial Load (11 tests)

> All tests use `MockKlineSource` to simulate exchange data

### 4.1 Initial Load

#### TC-G-001: test_initial_load_basic
- **Scenario**: 正常初始加载 — 1m + 高周期数据全部可用
- **Input**: Mock 提供 2000 根 1m + 各 1000 根高周期 (5m/15m/1h/4h/1d)
- **Expected**: 返回值 > 0，M1 candle_count > 0
- **Rationale**: 验证完整的初始加载流程

#### TC-G-002: test_initial_load_no_1m_data
- **Scenario**: 1m 数据为空
- **Input**: Mock 不提供任何数据
- **Expected**: 返回 0（无数据可加载）
- **Rationale**: 1m 是基础数据，没有 1m 则无法聚合高周期

#### TC-G-003: test_initial_load_1m_error
- **Scenario**: 获取 1m 数据时网络错误
- **Input**: Mock 对 "1m" 请求返回 error
- **Expected**: 返回 Err
- **Rationale**: 1m 数据获取失败应传播错误

#### TC-G-004: test_initial_load_high_tf_partial_failure
- **Scenario**: 高周期部分获取失败，1m 正常
- **Input**: 1m 成功，5m/1h/1d 成功，15m/4h 超时
- **Expected**: 返回值 > 0（1m 数据已加载，高周期部分降级）
- **Rationale**: 高周期获取失败不应阻塞 1m 数据加载，体现优雅降级

### 4.2 Gap Detection & Backfill

#### TC-G-005: test_no_gap_when_up_to_date
- **Scenario**: 缓存数据是最新的，无需回填
- **Input**: 缓存中有最近 1 分钟的 K 线
- **Expected**: 返回 0
- **Rationale**: 数据最新时不应发起不必要的网络请求

#### TC-G-006: test_gap_backfill
- **Scenario**: 缓存数据落后，存在缺口
- **Input**: 缓存中只有 10 分钟前的 K 线，Mock 提供 9 根缺口数据
- **Expected**: 返回值 > 0
- **Rationale**: 检测到缺口后应自动回填

#### TC-G-007: test_gap_backfill_empty_response
- **Scenario**: 缺口存在但交易所返回空数据
- **Input**: 缓存落后 10 分钟，Mock 返回空
- **Expected**: 返回 0
- **Rationale**: 交易所无数据时不应报错，返回 0 表示无新数据

### 4.3 Continuity Check

#### TC-G-008: test_check_continuity_empty_cache
- **Scenario**: 空缓存的连续性检查
- **Input**: 空 cache
- **Expected**: is_continuous=false, missing_minutes=u32::MAX
- **Rationale**: 空缓存视为完全不连续

#### TC-G-009: test_check_continuity_up_to_date
- **Scenario**: 数据最新的连续性检查
- **Input**: 缓存中有最近 1 分钟的 K 线
- **Expected**: is_continuous=true, missing_minutes=0
- **Rationale**: 数据最新时连续性应通过

#### TC-G-010: test_check_continuity_gap_detected
- **Scenario**: 检测到缺口
- **Input**: 缓存中只有 60 分钟前的 K 线
- **Expected**: is_continuous=false, missing_minutes>0, gap_start 和 gap_end 有值
- **Rationale**: 缺口检测应报告缺失范围

### 4.4 Event Broadcasting

#### TC-G-011: test_initial_load_event_broadcast
- **Scenario**: 初始加载完成后广播 Backfilled 事件
- **Input**: Mock 提供 100 根 1m 数据
- **Expected**: event_rx 收到事件，event_type=Backfilled, exchange="binance", symbol="BTCUSDT"
- **Rationale**: 外部系统（策略引擎、前端 WS）依赖事件通知

---

## 5. mod.rs — KlineEngine Integration (20 tests)

> All tests use `MockKlineSource` to simulate exchange data

### 5.1 Subscription Management

#### TC-E-001: test_subscribe_creates_entry
- **Scenario**: 订阅交易对，验证订阅成功
- **Input**: `subscribe("binance", "BTCUSDT", Spot)`
- **Expected**: 返回 Ok，is_subscribed=true
- **Rationale**: 订阅是使用引擎的第一步

#### TC-E-002: test_subscribe_idempotent
- **Scenario**: 重复订阅同一交易对
- **Input**: 连续两次 subscribe 同一 symbol
- **Expected**: 第二次返回 Ok（幂等）
- **Rationale**: 避免重复订阅导致数据重复

#### TC-E-003: test_subscribe_multiple_symbols
- **Scenario**: 订阅多个交易对
- **Input**: subscribe BTCUSDT + ETHUSDT
- **Expected**: 两者都 is_subscribed=true
- **Rationale**: 引擎需支持多 symbol 并行

#### TC-E-004: test_subscribe_backfills_data
- **Scenario**: 订阅时自动回填数据
- **Input**: subscribe 后查询 M1 数据
- **Expected**: M1 数据非空
- **Rationale**: backfill_on_start=true 时订阅应触发初始加载

#### TC-E-005: test_unsubscribe
- **Scenario**: 取消订阅
- **Input**: subscribe → unsubscribe
- **Expected**: is_subscribed 从 true 变为 false
- **Rationale**: 取消订阅应清理状态

#### TC-E-006: test_unsubscribe_nonexistent
- **Scenario**: 取消订阅不存在的交易对
- **Input**: unsubscribe 未订阅的 symbol
- **Expected**: 返回 Ok（不报错）
- **Rationale**: 幂等操作，不应崩溃

#### TC-E-007: test_subscribed_symbols
- **Scenario**: 获取已订阅列表
- **Input**: subscribe 2 个 symbol
- **Expected**: 列表长度=2
- **Rationale**: 管理界面需展示当前订阅

### 5.2 Data Query

#### TC-E-008: test_get_klines_unsubscribed
- **Scenario**: 查询未订阅的 symbol
- **Input**: 未订阅时 get_klines_async
- **Expected**: 返回 None
- **Rationale**: 未订阅不应返回数据

#### TC-E-009: test_get_all_timeframes
- **Scenario**: 获取所有周期数据
- **Input**: subscribe 后 get_all_timeframes
- **Expected**: 返回 Some，m1 非空
- **Rationale**: 前端一次性获取所有周期

### 5.3 Event System

#### TC-E-010: test_subscribe_events
- **Scenario**: 订阅事件广播
- **Input**: `subscribe_events()`
- **Expected**: 返回 Receiver（不 panic）
- **Rationale**: 事件广播是引擎的核心输出

### 5.4 Continuity & Backfill

#### TC-E-011: test_continuity_check
- **Scenario**: 已订阅 symbol 的连续性检查
- **Input**: subscribe 后 continuity_check
- **Expected**: 返回 Some(ContinuityReport)
- **Rationale**: 连续性检查是数据质量保障

#### TC-E-012: test_continuity_check_unsubscribed
- **Scenario**: 未订阅 symbol 的连续性检查
- **Input**: 未订阅时 continuity_check
- **Expected**: 返回 None
- **Rationale**: 未订阅无法检查连续性

#### TC-E-013: test_force_backfill
- **Scenario**: 强制回填已订阅 symbol
- **Input**: subscribe 后 force_backfill
- **Expected**: 返回 Ok
- **Rationale**: 手动触发回填用于修复数据

#### TC-E-014: test_force_backfill_unsubscribed
- **Scenario**: 强制回填未订阅 symbol
- **Input**: 未订阅时 force_backfill
- **Expected**: 返回 Err
- **Rationale**: 未订阅无法回填

### 5.5 Backtest Utilities

#### TC-E-015: test_backtest_range_limits
- **Scenario**: 获取所有周期的回测范围限制
- **Input**: `backtest_range_limits()`
- **Expected**: 返回 6 个限制配置
- **Rationale**: 回测系统需知道各周期的最大范围

#### TC-E-016: test_validate_backtest_range_valid
- **Scenario**: 验证合法的回测范围
- **Input**: M1, 3 天
- **Expected**: Ok
- **Rationale**: 3 天在 M1 的推荐范围内

#### TC-E-017: test_validate_backtest_range_exceeds_max
- **Scenario**: 验证超出最大范围的回测请求
- **Input**: M1, 100 天
- **Expected**: Err
- **Rationale**: 100 天远超 M1 的 7 天上限，应拒绝

#### TC-E-018: test_calculate_fetch_limit
- **Scenario**: 计算需要获取的 K 线数量
- **Input**: M1, start=0, end=3600000 (1小时)
- **Expected**: limit=60
- **Rationale**: 1 小时 = 60 根 1m K 线

#### TC-E-019: test_subscription_key_format
- **Scenario**: 验证订阅键格式
- **Input**: `subscription_key("binance", "BTCUSDT")`
- **Expected**: `"binance:BTCUSDT"`
- **Rationale**: 订阅键格式一致性

#### TC-E-020: test_is_subscribed_sync
- **Scenario**: 同步查询订阅状态（未订阅）
- **Input**: 新引擎，查询 is_subscribed
- **Expected**: false
- **Rationale**: 初始状态应为未订阅

---

## Appendix: Test Execution

```
$ cargo test --bin virs kline 2>&1 | grep "test result"
test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out
```

### Coverage Summary by Category

| Category | Tests | Key Invariants Verified |
|----------|-------|------------------------|
| Timeframe 枚举 | 8 | 毫秒/分钟/字符串转换、serde、默认限制 |
| Candle 数据 | 5 | merge 规则 (OHLCV)、from_1m 对齐、闭合状态 |
| 工具函数 | 6 | 时间对齐、订阅键格式、WS symbol 转换、事件序列化、回测限制、引擎配置 |
| 批量聚合 | 15 | 各周期 OHLCV 计算、空/单/部分输入、跨边界、缺口处理 |
| 增量更新 | 6 | 首根创建、周期闭合、新周期开始 |
| 缓存 CRUD | 6 | 追加/覆盖/历史更新、闭合标记 |
| 缓存检索 | 2 | last_closed/last 查询 |
| 缓存批量 | 7 | replace/backfill、截断、覆盖优先级 |
| 缓存边界 | 5 | 容量限制、空查询、独立性、顺序保持 |
| 初始加载 | 4 | 正常/无数据/错误/部分失败 |
| 缺口处理 | 3 | 最新/有缺口/空响应 |
| 连续性检查 | 3 | 空/最新/有缺口 |
| 事件广播 | 1 | Backfilled 事件 |
| 订阅管理 | 7 | 创建/幂等/多 symbol/回填/取消/列表 |
| 数据查询 | 2 | 已订阅/未订阅 |
| 事件系统 | 1 | 事件订阅 |
| 连续性/回填 | 4 | 检查/强制回填/未订阅 |
| 回测工具 | 4 | 范围限制/验证/计算 |
| 杂项 | 2 | 订阅键/同步查询 |
