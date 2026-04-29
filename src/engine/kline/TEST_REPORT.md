# K 线模块测试报告

> 最后更新：2026-04-27 | 测试总数：116 个（已按审计建议优化）

## 一、模块总览

| 模块 | 文件 | 测试数 | 覆盖质量 |
|------|------|--------|---------|
| 核心类型 | `types.rs` | 15 | ⭐⭐⭐⭐⭐ |
| K 线缓存 | `cache.rs` | 21 | ⭐⭐⭐⭐⭐ |
| 周期聚合 | `aggregator.rs` | 18 | ⭐⭐⭐⭐ |
| 缺口检测 | `gap.rs` | 13 | ⭐⭐⭐⭐ |
| 引擎集成 | `mod.rs::tests` | 20 | ⭐⭐⭐⭐ |
| 数据一致性 | `mod.rs::consistency_tests` | 11 | ⭐⭐⭐⭐ |
| 交易所 WS | `binance_kline_ws.rs` | 11 | ⭐⭐⭐⭐ |
| 前端 WS/API | `api/kline.rs` | 7 | ⭐⭐⭐ |
| **合计** | | **116** | |

---

## 二、优化记录

### 2.1 合并（减少 13 个测试）

| 组 | 合并前 | 合并后 | 文件 |
|----|--------|--------|------|
| A | `test_timeframe_minutes` + `test_timeframe_ms` | `test_timeframe_ms` | `types.rs` |
| B | `test_timeframe_display` + `test_timeframe_as_str` | `test_timeframe_as_str` | `types.rs` |
| C | `test_candle_from_1m_alignment` + `test_candle_from_1m` | `test_candle_from_1m` | `types.rs` |
| D | `test_backfill_truncation` + `test_replace_timeframe_truncation` | `test_replace_timeframe_truncation` | `cache.rs` |
| E | 3 个 `test_is_last_1m_in_group_*` | `test_is_last_1m_in_group` | `aggregator.rs` |
| F | `test_aggregate_high_low_tracking` + `test_aggregate_volume_accumulation` | `test_aggregate_5m_single_group` | `aggregator.rs` |
| G | `test_get_klines_async_unsubscribed` + `test_get_klines_unsubscribed` | `test_get_klines_unsubscribed` | `mod.rs` |
| H | `test_parse_binance_kline_closed` + `test_parse_binance_kline_message` | `test_parse_binance_kline_message` | `binance_kline_ws.rs` |
| I | `test_to_candle_closed` + `test_to_candle_basic` | `test_to_candle_basic` | `binance_kline_ws.rs` |
| J | 3 个 symbol 转换测试 | `test_binance_ws_symbol_basic` | `binance_kline_ws.rs` |
| K | 3 个 KlineEvent 序列化测试 | `test_kline_event_serialization` | `api/kline.rs` |
| L | `test_broadcast_lagged_then_closed` + `test_broadcast_lagged_recovery` | `test_broadcast_lagged_recovery` | `api/kline.rs` |

### 2.2 删除（3 个测试）

| 测试 | 原因 |
|------|------|
| `test_subscribe_events` | 无实际断言，仅验证不 panic |
| `test_subscribe_events_multiple_receivers` | 无实际断言 |
| `test_event_broadcast_on_backfill` | 与 `gap.rs::test_initial_load_event_broadcast` 重复 |

### 2.3 新增 P0 测试（6 个）

| # | 测试名 | 文件 | 覆盖场景 |
|---|--------|------|----------|
| 1 | `test_fetch_backtest_data_cache_hit` | `mod.rs` | `fetch_backtest_data` 缓存命中路径 |
| 2 | `test_fetch_backtest_data_unsubscribed` | `mod.rs` | `fetch_backtest_data` 未订阅返回 None |
| 3 | `test_start_stop_lifecycle` | `mod.rs` | `start()→stop()→start()` 生命周期 |
| 4 | `test_gap_backfill_truncation_large_gap` | `gap.rs` | `gap_minutes.min(1000)` 截断逻辑 |
| 5 | `test_initial_load_unclosed_high_tf_replacement` | `gap.rs` | `initial_load` unclosed 高级周期替换 |
| 6 | `test_update_higher_timeframes_stale_data_branch` | `aggregator.rs` | `aligned_open < last_candle.open_time` 返回 None |

---

## 三、各模块测试详情

### 1. types.rs — 核心类型（15 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_timeframe_ms` | 周期毫秒+分钟转换 | `M1, M5, M15, H1, H4, D1` | ms + minutes 值正确 | 验证每个周期映射到正确的毫秒和分钟时长 |
| 2 | `test_timeframe_as_str` | 周期字符串+Display | 所有周期 | `"1m", "5m", ..., "1d"` | 确保 as_str 和 Display 输出一致 |
| 3 | `test_timeframe_from_str_lossy` | 周期字符串解析 | `"1m", "5m", "1D", "2h", ""` | 对应 `Option<Timeframe>` 值 | 验证往返解析和边界情况 |
| 4 | `test_timeframe_default_limit` | 默认 K 线数量限制 | 所有周期 | `M1=2000`，其余=`1000` | 确保各周期的 API 请求限制合理 |
| 5 | `test_timeframe_all` | 所有周期枚举 | `Timeframe::all()` | 6 个元素，包含首尾 | 验证周期覆盖完整 |
| 6 | `test_timeframe_serde` | JSON 序列化 | `Timeframe::M1` | `"\"1m\""` | 验证 serde 往返兼容性 |
| 7 | `test_market_type_display` | MarketType Display | `Spot, Perpetual` | `"spot", "perpetual"` | 确保人类可读输出 |
| 8 | `test_market_type_from_str_lossy` | MarketType 解析 | `"spot", "perpetual", "swap", "SPOT"` | 对应变体 | 验证别名处理和大小写不敏感 |
| 9 | `test_candle_merge` | K 线合并逻辑 | 基础 K 线 + 更新 K 线 | `high=115.0, low=90.0, close=108.0, volume=80.0` | 验证增量 K 线更新合并正确 |
| 10 | `test_candle_from_1m` | 1m K 线转换+对齐 | `open_time=3600000ms` 和 `3630000ms` | H1 K 线，`open_time=3600000` | 验证周期对齐和边界对齐 |
| 11 | `test_align_open_time` | 时间对齐函数 | 各种 open_time + 周期组合 | 对齐到周期边界 | K 线时间标准化的核心工具 |
| 12 | `test_subscription_key` | 订阅键格式 | `"Binance", "btcusdt"` | `"binance:BTCUSDT"` | 验证键名大小写标准化 |
| 13 | `test_kline_event_type_serde` | KlineEventType 序列化 | `Update, Closed, Backfilled` | 对应 JSON 字符串 | 确保事件类型正确序列化 |
| 14 | `test_backtest_range_limit` | 回测范围限制 | `M1, D1` | `M1: max=7, rec=3`; `D1: max=1825` | 验证各周期的合理默认值 |
| 15 | `test_kline_engine_config_default` | 默认配置值 | `KlineEngineConfig::default()` | `backfill_on_start=true, capacity=8192` | 验证安全的默认值 |

### 2. cache.rs — SymbolCache（21 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_symbol_cache_new` | 空缓存初始化 | `SymbolCache::new()` | 所有周期数量 = 0 | 验证干净状态 |
| 2 | `test_update_candle_append` | 追加 K 线 | 两根连续 M1 K 线 | `candle_count(M1) == 2` | 基础追加功能 |
| 3 | `test_update_candle_same_open_time_overwrite` | 相同 open_time 覆盖 | 两根 `open_time=0` 的 K 线 | 数量 = 1，`open=110.0, closed=true` | 验证覆盖语义 |
| 4 | `test_update_candle_older_update_in_place` | 乱序更新 | 三根 K 线：0, 60000，然后更新 0 | 数量 = 2，K 线 0 原地更新 | 处理延迟到达的数据 |
| 5 | `test_close_candle` | 闭合 K 线 | 未闭合 K 线 | `closed=true` | 标记 K 线完成 |
| 6 | `test_close_candle_nonexistent` | 闭合不存在的 K 线 | `close_candle(M1, 999)` | 无报错 | 优雅处理缺失 K 线 |
| 7 | `test_last_closed_1m` | 最后闭合的 1m K 线 | 未闭合 + 已闭合 K 线 | 返回已闭合的 K 线 | 追踪最后完成的 K 线 |
| 8 | `test_last_1m` | 最后的 1m K 线 | 未闭合 + 已闭合 K 线 | 返回最新的 K 线（不限状态） | 追踪最新 K 线 |
| 9 | `test_replace_timeframe` | 替换所有 K 线 | 10 根新 K 线 | `candle_count(M1) == 10` | 批量替换用于回填 |
| 10 | `test_replace_timeframe_truncation` | 替换+回填容量限制 | `limit + 100` 根 K 线 | 数量 = `limit` | 防止内存无限增长（含回填截断） |
| 11 | `test_backfill_timeframe_empty_cache` | 空缓存回填 | 5 根 K 线 | 数量 = 5 | 初始数据加载 |
| 12 | `test_backfill_timeframe_append_new` | 回填追加新 K 线 | 已有 2 根 + 3 根新 K 线 | 数量 = 5 | 缺口填充 |
| 13 | `test_backfill_overwrite_unclosed_with_closed` | 未闭合升级为已闭合 | 未闭合 K 线 + 已闭合回填 | `closed=true` | 确保已闭合数据优先 |
| 14 | `test_backfill_no_overwrite_closed_with_closed` | 已闭合不被覆盖 | 已闭合 K 线 + 不同的已闭合回填 | 原始保留 | 永不覆盖已确认数据 |
| 15 | `test_backfill_no_overwrite_closed_with_unclosed` | 已闭合不被未闭合覆盖 | 已闭合 K 线 + 未闭合回填 | 原始保留 | 防止数据回退 |
| 16 | `test_capacity_enforcement_on_update` | 单次更新的最大容量 | `limit + 50` 次连续更新 | 数量 = `limit` | 连续更新保持有界 |
| 17 | `test_get_all_timeframes` | 所有周期快照 | M1 + M5 K 线 | `m1.len()=1, m5.len()=1` | 完整数据导出 |
| 18 | `test_get_klines_nonexistent_timeframe` | 空周期查询 | 空缓存 | 空数组 | 缺失数据的安全默认值 |
| 19 | `test_is_empty_after_data` | 非空检测 | 添加一根 K 线 | `is_empty() == false` | 状态追踪 |
| 20 | `test_multiple_timeframes_independent` | 周期隔离 | M1: 2 根，M5: 1 根 | `M1=2, M5=1, H1=0` | 周期之间互不干扰 |
| 21 | `test_timeframe_buffer_push_or_update_order_preserved` | 插入顺序 | 3 根连续 K 线 | 顺序：0, 60000, 120000 | 时间顺序保持 |

### 3. aggregator.rs — 周期聚合（18 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_aggregate_5m_single_group` | 基础 5m 聚合+H/L/V | 5 根已闭合 1m K 线 | 1 根已闭合 5m，OHLCV 正确 | 核心聚合逻辑（含最高/最低/成交量） |
| 2 | `test_aggregate_two_5m_groups` | 多组 5m 聚合 | 10 根已闭合 1m K 线 | 2 根已闭合 5m | 多窗口处理 |
| 3 | `test_aggregate_empty` | 空输入 | 无 K 线 | 空结果 | 优雅处理空输入 |
| 4 | `test_aggregate_single_candle` | 单根 K 线 | 1 根已闭合 1m K 线 | 1 根未闭合 5m | 最小输入 |
| 5 | `test_aggregate_partial_group_unclosed` | 不完整窗口 | 3 根已闭合 1m K 线 | 1 根未闭合 5m | 处理不完整窗口 |
| 6 | `test_aggregate_15m` | 15m 聚合 | 15 根已闭合 1m K 线 | 1 根已闭合 15m | 多步聚合 |
| 7 | `test_aggregate_1h` | 1h 聚合 | 60 根已闭合 1m K 线 | 1 根已闭合 H1 | 小时级 K 线构建 |
| 8 | `test_aggregate_4h` | 4h 聚合 | 240 根已闭合 1m K 线 | 1 根已闭合 H4 | 扩展周期 |
| 9 | `test_aggregate_1d` | 1d 聚合 | 1440 根已闭合 1m K 线 | 1 根已闭合 D1 | 日 K 线构建 |
| 10 | `test_aggregate_1d_two_days` | 跨日 1d 聚合 | 2880 根已闭合 1m K 线 | 2 根已闭合 D1 | 多日聚合 |
| 11 | `test_aggregate_mixed_closed_unclosed` | 混合闭合状态 | 3 根已闭合 + 2 根未闭合 | 1 根未闭合 5m | 未闭合状态传播 |
| 12 | `test_is_last_1m_in_group` | 组末尾判断（多周期） | 5m/1h/1d 组末尾 1m | `true/false` | 聚合触发判断（参数化） |
| 13 | `test_update_higher_timeframes_first_candle` | 首根 1m 更新 | 空缓存 + 1 根 1m | 所有高级周期有未闭合 K 线 | 缓存初始化 |
| 14 | `test_update_higher_timeframes_closing_5m` | 5m 关闭 | 5m 组最后一根 1m | M5 标记已闭合 | 周期闭合传播 |
| 15 | `test_update_higher_timeframes_new_period` | 新周期开始 | 5m 关闭后下一根 1m | 新的未闭合 5m K 线 | 周期切换 |
| 16 | `test_update_higher_timeframes_stale_data_branch` | 旧数据分支 | `aligned_open < last_candle.open_time` | 返回 `None` | 防止旧数据污染新周期 |
| 17 | `test_aggregate_non_aligned_start` | 非对齐起始 | 非整点时间的 1m K 线 | 对齐到周期边界 | 时间标准化 |
| 18 | `test_aggregate_gap_in_1m_data` | 数据缺口 | 间隔 10 分钟的两根 1m | 2 个独立窗口 | 缺口处理 |

### 4. gap.rs — 缺口检测与回填（13 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_initial_load_basic` | 初始数据加载 | 2000 根 1m + 1000 根高级周期 K 线 | `result > 0`，M1 数量 > 0 | 完整初始加载路径 |
| 2 | `test_initial_load_no_1m_data` | 无 1m 数据 | 空 mock 数据源 | `result == 0` | 优雅处理空数据源 |
| 3 | `test_initial_load_1m_error` | 1m 获取失败 | Mock 返回错误 | `result.is_err()` | 错误传播 |
| 4 | `test_initial_load_high_tf_partial_failure` | 高级周期部分失败 | 部分周期返回错误 | `result > 0`（部分成功） | 对部分失败的容错性 |
| 5 | `test_no_gap_when_up_to_date` | 无缺口场景 | 缓存中有最近 K 线 | `result == 0` | 跳过不必要的回填 |
| 6 | `test_gap_backfill` | 缺口检测和填充 | 旧 K 线 + 缺口数据 | `result > 0` | 缺口填充功能 |
| 7 | `test_gap_backfill_empty_response` | 缺口但无数据源 | 旧 K 线，空数据源 | `result == 0` | 处理缺失的缺口数据 |
| 8 | `test_check_continuity_empty_cache` | 空缓存连续性 | 空缓存 | `is_continuous=false` | 检测缺失数据 |
| 9 | `test_check_continuity_up_to_date` | 数据最新时连续性 | 最近的已闭合 K 线 | `is_continuous=true` | 确认数据新鲜度 |
| 10 | `test_check_continuity_gap_detected` | 缺口检测 | 60 分钟前的 K 线 | `is_continuous=false, missing_minutes > 0` | 识别数据缺口 |
| 11 | `test_initial_load_event_broadcast` | 初始加载事件广播 | 100 根 1m K 线 | 收到 `Backfilled` 事件 | 通知订阅者数据加载 |
| 12 | `test_gap_backfill_truncation_large_gap` | 大间隔回填截断 | 缺口 > 1000 分钟 | 请求限制 = 1000 | 验证 `gap_minutes.min(1000)` 截断 |
| 13 | `test_initial_load_unclosed_high_tf_replacement` | unclosed 高级周期替换 | unclosed 高级周期 K 线 | 被 closed 数据替换 | 验证 initial_load 替换逻辑 |

### 5. mod.rs — KlineEngine 集成测试（20 个）

#### 5.1 订阅管理（7 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_subscribe_creates_entry` | 基础订阅 | `subscribe("binance", "BTCUSDT", Spot)` | `is_subscribed == true` | 验证订阅创建内部条目 |
| 2 | `test_subscribe_idempotent` | 重复订阅 | 连续两次 subscribe | `len() == 1` | 防止重复订阅 |
| 3 | `test_subscribe_multiple_symbols` | 多交易对订阅 | BTCUSDT + ETHUSDT | `len() == 2` | 独立订阅追踪 |
| 4 | `test_subscribe_spot_calls_spot_ws` | 现货 WS 委托 | `subscribe(..., Spot)` | spot_ws 包含 `"BTCUSDT"` | 现货路由到现货 WS |
| 5 | `test_subscribe_perpetual_calls_perpetual_ws` | 永续 WS 委托 | `subscribe(..., Perpetual)` | perpetual_ws 包含 `"BTCUSDT"` | 永续路由到永续 WS |
| 6 | `test_unsubscribe` | 取消订阅 | subscribe → unsubscribe | `is_subscribed == false` | 清理订阅条目 |
| 7 | `test_unsubscribe_nonexistent` | 取消不存在订阅 | unsubscribe 未订阅的 symbol | `Ok(())` | 优雅处理无效取消 |

#### 5.2 数据查询（3 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 8 | `test_get_klines_unsubscribed` | 查询未订阅（同步+异步） | 未订阅时 `get_klines` / `get_klines_async` | `None` | 防止无效查询 |
| 9 | `test_get_klines_after_subscribe_with_backfill` | 订阅后回填数据 | subscribe（backfill 开启）+ Mock 10 根 1m | `Some, len() > 0` | 验证数据从源流经缓存 |
| 10 | `test_get_all_timeframes` | 全周期数据 | subscribe（回填开启）+ Mock | `Some` | 完整周期快照可用 |

#### 5.3 回测工具（4 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 11 | `test_backtest_range_limits` | 范围限制枚举 | `backtest_range_limits()` | 6 个限制配置 | 所有周期完整覆盖 |
| 12 | `test_validate_backtest_range_valid` | 有效范围 | `validate_backtest_range(M1, 3)` | `Ok(())` | 在推荐范围内 |
| 13 | `test_validate_backtest_range_exceeds_max` | 超出最大值 | `validate_backtest_range(M1, 100)` | `Err` | 防止过多数据请求 |
| 14 | `test_validate_backtest_range_recommended_warning` | 警告范围 | `validate_backtest_range(M1, 5)` | `Ok(())` + warning | 大范围的提示 |

#### 5.4 连续性/回填（2 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 15 | `test_continuity_check_unsubscribed` | 未订阅连续性检查 | 未订阅时 `continuity_check` | `None` | 防止无效检查 |
| 16 | `test_force_backfill_unsubscribed` | 未订阅强制回填 | 未订阅时 `force_backfill` | `Err` | 防止对未知交易对回填 |

#### 5.5 回测数据获取（2 个）🆕

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 17 | `test_fetch_backtest_data_cache_hit` | 缓存命中 | 已订阅 + 有缓存数据 | 返回缓存数据 | 验证缓存优先路径 |
| 18 | `test_fetch_backtest_data_unsubscribed` | 未订阅 | 未订阅时调用 | `None` | 防止无效回测请求 |

#### 5.6 生命周期（1 个）🆕

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 19 | `test_start_stop_lifecycle` | 启停生命周期 | `start()→stop()→start()` | 两次 start 均成功 | 验证引擎可重启 |

### 6. mod.rs — 数据一致性测试（11 个）

#### 6.1 REST 一致性（6 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_rest_consistency_m1` | 1m REST 数据一致性 | Mock 提供 100 根 1m → 引擎 backfill → 查询 M1 | 100 根逐根一致 | 验证 REST 加载无数据损失 |
| 2 | `test_rest_consistency_m5` | 5m 聚合一致性 | 100 根 1m → 引擎聚合 → 与 `Aggregator` 预聚合比对 | 20 根 5m 逐根一致 | 验证聚合算法一致性 |
| 3 | `test_rest_consistency_m15` | 15m 聚合一致性 | 150 根 1m → 同上 | 10 根 15m 逐根一致 | 验证 15m 聚合 |
| 4 | `test_rest_consistency_h1` | 1h 聚合一致性 | 120 根 1m → 同上 | 2 根 H1 逐根一致 | 验证 1h 聚合 |
| 5 | `test_rest_consistency_h4` | 4h 聚合一致性 | 240 根 1m → 同上 | 1 根 H4 逐根一致 | 验证 4h 聚合 |
| 6 | `test_rest_consistency_d1` | 1d 聚合一致性 | 1440 根 1m → 同上 | 1 根 D1 逐根一致 | 验证 1d 聚合 |

#### 6.2 WS 一致性（4 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 7 | `test_ws_consistency_m1_realtime` | M1 实时推送 | Mock WS 推送 50 根 1m → 逐根比对事件 | 50 根逐根一致 | 验证 WS 推送无数据损失 |
| 8 | `test_ws_consistency_m5_aggregated` | M5 聚合推送 | 60 根 1m → 收集 M5 事件 → 与预聚合比对 | 12 根 M5 逐根一致 | 验证 WS 聚合一致性 |
| 9 | `test_ws_consistency_update_then_close` | Update→Close 流 | 3 次 update + 1 次 close | 4 个事件，最后一次 `closed=true` | 验证事件流状态转换 |
| 10 | `test_ws_consistency_multi_timeframe` | 多周期同时聚合 | 240 根 1m → M1/M5/M15/H1 | 各周期 closed 事件与预聚合一致 | 验证多周期并发聚合 |

#### 6.3 端到端一致性（2 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 11 | `test_e2e_rest_then_ws` | REST + WS 端到端 | REST 50 根 + WS 10 根 | 60 根逐根一致 | 验证 REST→WS 数据衔接 |
| 12 | `test_e2e_ws_reconnect_data_integrity` | WS 重连完整性 | 20 根 + 重连 + 20 根 | 40 根逐根一致 | 验证重连后数据不丢失 |

### 7. binance_kline_ws.rs — 交易所 WebSocket 客户端（11 个）

#### 7.1 消息解析（5 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_parse_binance_kline_message` | 标准+已闭合消息 | Binance WS JSON（closed=false/true） | `stream/data` 正确解析，`closed` 标志正确 | 验证标准消息格式和 closed 标志 |
| 2 | `test_parse_binance_kline_message_without_stream` | 无 stream 字段 | `{"data":{...}}` | `stream=None, data!=None` | 兼容无 stream 格式 |
| 3 | `test_parse_invalid_json` | 无效 JSON | `"not json"` | 反序列化 `Err` | 防御无效输入 |
| 4 | `test_parse_non_kline_event` | 非 kline 事件 | `{"data":{"e":"trade",...}}` | 反序列化 `Err`（缺 k 字段） | 非 kline 事件不被处理 |

#### 7.2 Candle 转换（3 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 5 | `test_to_candle_basic` | 基础+已闭合转换 | 完整 BinanceKlineData（closed=false/true） | `Candle` 字段一一对应，`closed` 标志正确 | 验证字段映射和 closed 传递 |
| 6 | `test_to_candle_invalid_numbers` | 无效数字 | `o="not_a_number"` | `open=0.0` | 解析失败回退 |
| 7 | `test_ws_symbol` | symbol 提取 | `k.s="BTCUSDT"` | `"BTCUSDT"` | 正确提取交易对 |

#### 7.3 Symbol 转换（1 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 8 | `test_binance_ws_symbol_basic` | 多场景 symbol 转换 | `"BTCUSDT"`, `"BTC/USDT"`, `"btcusdt"` | 均为 `"btcusdt"` | 小写化、去斜杠、幂等性 |

#### 7.4 构造函数和状态（3 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 9 | `test_new_spot` | 现货构造 | `new_spot(None)` | `ws_url` 正确，`running=false` | 验证现货 URL |
| 10 | `test_new_perpetual` | 永续构造 | `new_perpetual(None)` | `ws_url` 正确，`running=false` | 验证永续 URL |
| 11 | `test_subscribe_without_start` | 未启动时订阅 | 不 start 直接 subscribe | subscriptions 包含 stream 名 | 未连接时订阅不崩溃 |

### 8. api/kline.rs — 前端 WebSocket Handler（7 个）

#### 8.1 事件序列化（1 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_kline_event_serialization` | 多类型序列化 | `Update/Closed/Backfilled` | JSON 含正确 `event_type` | 验证 WS 推送格式（参数化） |

#### 8.2 Broadcast Channel 行为（3 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 2 | `test_broadcast_lagged_recovery` | Lagged 恢复+关闭 | `capacity=2`，发 5 条，后 drop sender | Lagged 后继续接收，最终 Closed | 验证 Lagged 不导致停止 |
| 3 | `test_broadcast_multiple_receivers` | 多接收器 | 3 个 receiver | 都收到消息 | 验证扇出能力 |
| 4 | `test_broadcast_receiver_lagged_independently` | 独立 Lagged | 2 个 receiver 不消费 | 都收到 Lagged | 接收器互不影响 |

#### 8.3 API 响应序列化（3 个）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 5 | `test_kline_response_serialization` | 成功响应 | `KlineResponse(success=true)` | JSON 序列化成功 | 验证 HTTP 响应格式 |
| 6 | `test_kline_response_error` | 错误响应 | `KlineResponse(success=false)` | JSON 含 `"success":false` | 验证错误响应格式 |
| 7 | `test_all_timeframes_response` | 全周期响应 | `AllTimeframesData` | 通过 `KlineData::AllTimeframes` 序列化 | 验证多周期数据格式 |

---

## 四、剩余欠缺测试（按优先级）

### P1 — 中优先级

| # | 模块 | 缺失场景 | 说明 |
|---|------|----------|------|
| 1 | `api/kline.rs` | HTTP handler 集成测试 | 所有 handler 无 HTTP 层测试 |
| 2 | `api/kline.rs` | engine 未初始化时的 handler 返回 | `kline_engine None` 检查无测试 |
| 3 | `cache.rs` | 并发安全测试 | 多线程同时 update/read 无测试 |
| 4 | `gap.rs` | 高级周期全部失败 | 仅测了部分失败 |
| 5 | `mod.rs` | 多 symbol 并发订阅 | 无并发场景测试 |

### P2 — 低优先级

| # | 模块 | 缺失场景 | 说明 |
|---|------|----------|------|
| 6 | `types.rs` | `MarketType` serde 测试 | MarketType 有 Serialize/Deserialize 但无 serde 测试 |
| 7 | `binance_ws.rs` | `unsubscribe()` 功能 | 取消订阅后列表更新无测试 |
| 8 | `aggregator.rs` | 跨日/跨时区聚合 | 所有测试从时间 0 开始 |
| 9 | `cache.rs` | 乱序 backfill | 当前 backfill 都是顺序数据 |

---

## 五、优化前后对比

| 指标 | 优化前 | 优化后 | 变化 |
|------|--------|--------|------|
| 测试总数 | 117 | 116 | -1 |
| P0 缺口 | 5 个 | 0 个 | ✅ 全部补齐 |
| 重复/冗余 | 15 个合并 + 3 个删除 | 0 | ✅ 全部清理 |
| 覆盖质量 | 数据层优秀，系统层不足 | 系统层基本覆盖 | ⬆️ 提升 |
| 维护性 | 存在碎片化重复 | 合并后更清晰 | ⬆️ 提升 |
