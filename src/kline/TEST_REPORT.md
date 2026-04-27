# K 线模块测试报告

## 1. types.rs — 核心类型（18 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_timeframe_ms` | 周期毫秒转换 | `M1`, `M5`, `M15`, `H1`, `H4`, `D1` | `60000`, `300000`, `900000`, `3600000`, `14400000`, `86400000` | 验证每个周期映射到正确的毫秒时长 |
| 2 | `test_timeframe_as_str` | 周期字符串表示 | `M1`, `M5`, `M15`, `H1`, `H4`, `D1` | `"1m"`, `"5m"`, `"15m"`, `"1h"`, `"4h"`, `"1d"` | 确保字符串输出符合交易所 API 约定 |
| 3 | `test_timeframe_from_str_lossy` | 周期字符串解析 | `"1m"`, `"5m"`, `"15m"`, `"1h"`, `"4h"`, `"1d"`, `"1D"`, `"2h"`, `""` | 对应的 `Option<Timeframe>` 值 | 验证往返解析和边界情况 |
| 4 | `test_timeframe_minutes` | 周期分钟转换 | `M1`, `M5`, `M15`, `H1`, `H4`, `D1` | `1`, `5`, `15`, `60`, `240`, `1440` | 验证分钟级粒度计算 |
| 5 | `test_timeframe_default_limit` | 各周期默认 K 线数量限制 | 所有周期 | `M1=2000`，其余=`1000` | 确保各周期的 API 请求限制合理 |
| 6 | `test_timeframe_all` | 所有周期枚举 | `Timeframe::all()` | 6 个元素，包含所有变体 | 验证周期覆盖完整 |
| 7 | `test_timeframe_display` | Display trait 实现 | `M1`, `H1`, `D1` | `"1m"`, `"1h"`, `"1d"` | 确保 Display 输出与 as_str 一致 |
| 8 | `test_timeframe_serde` | JSON 序列化 | `Timeframe::M1` | `"\"1m\""` | 验证 serde 往返兼容性 |
| 9 | `test_market_type_display` | MarketType Display trait | `Spot`, `Perpetual` | `"spot"`, `"perpetual"` | 确保人类可读输出 |
| 10 | `test_market_type_from_str_lossy` | MarketType 字符串解析 | `"spot"`, `"perpetual"`, `"swap"`, `"future"`, `"SPOT"` | 对应变体 | 验证别名处理和大小写不敏感 |
| 11 | `test_candle_merge` | K 线合并逻辑 | 基础 K 线 + 更新 K 线 | 合并后：`high=115.0`, `low=90.0`, `close=108.0`, `volume=80.0` | 验证增量 K 线更新合并正确 |
| 12 | `test_candle_from_1m` | 从 1m K 线转换 | `open_time=3600000ms` 的 1m K 线 | H1 K 线，`open_time=3600000` | 验证周期对齐 |
| 13 | `test_candle_from_1m_alignment` | 开盘时间对齐 | `open_time=3630000ms` 的 1m K 线 | H1 K 线，`open_time=3600000` | 验证对齐到周期边界 |
| 14 | `test_align_open_time` | 时间对齐函数 | 各种 open_time + 周期组合 | 对齐到周期边界 | K 线时间标准化的核心工具 |
| 15 | `test_subscription_key` | 订阅键格式 | `"Binance"`, `"btcusdt"` | `"binance:BTCUSDT"` | 验证键名大小写标准化 |
| 16 | `test_kline_event_type_serde` | KlineEventType 序列化 | `Update`, `Closed`, `Backfilled` | 对应 JSON 字符串 | 确保事件类型正确序列化 |
| 17 | `test_backtest_range_limit` | 回测范围限制 | `M1`, `D1` | `M1: max=7, rec=3`; `D1: max=1825` | 验证各周期的合理默认值 |
| 18 | `test_kline_engine_config_default` | 默认配置值 | `KlineEngineConfig::default()` | `backfill_on_start=true`, `capacity=8192` | 验证安全的默认值 |

## 2. cache.rs — SymbolCache（22 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_symbol_cache_new` | 空缓存初始化 | `SymbolCache::new()` | 所有周期数量 = 0，`is_empty()=true` | 验证干净状态 |
| 2 | `test_update_candle_append` | 追加 K 线 | 两根连续 M1 K 线 | `candle_count(M1) == 2` | 基础追加功能 |
| 3 | `test_update_candle_same_open_time_overwrite` | 相同 open_time 更新 | 两根 `open_time=0` 的 K 线 | 数量 = 1，`open=110.0`，`closed=true` | 验证覆盖语义 |
| 4 | `test_update_candle_older_update_in_place` | 乱序更新 | 三根 K 线：0, 60000，然后更新 0 | 数量 = 2，K 线 0 原地更新 | 处理延迟到达的数据 |
| 5 | `test_close_candle` | 闭合 K 线 | time=0 的未闭合 K 线 | `close_candle` 后 `closed=true` | 标记 K 线完成 |
| 6 | `test_close_candle_nonexistent` | 闭合不存在的 K 线 | `close_candle(M1, 999)` | 数量 = 0，无报错 | 优雅处理缺失 K 线 |
| 7 | `test_last_closed_1m` | 最后闭合的 1m K 线 | 未闭合 + 已闭合 K 线 | 返回 time=0 的 K 线 | 追踪最后完成的 K 线 |
| 8 | `test_last_1m` | 最后的 1m K 线（任意状态） | 未闭合 + 已闭合 K 线 | 返回 time=60000 的 K 线 | 追踪最新 K 线（不限状态） |
| 9 | `test_replace_timeframe` | 替换所有 K 线 | 10 根新 K 线 | `candle_count(M1) == 10` | 批量替换用于回填 |
| 10 | `test_replace_timeframe_truncation` | 替换时容量限制 | `limit + 100` 根 K 线 | 数量 = `limit`，最旧的被丢弃 | 防止内存无限增长 |
| 11 | `test_backfill_timeframe_empty_cache` | 空缓存回填 | 5 根 K 线 | 数量 = 5 | 初始数据加载 |
| 12 | `test_backfill_timeframe_append_new` | 回填追加新 K 线 | 已有 2 根 + 3 根新 K 线 | 数量 = 5 | 缺口填充 |
| 13 | `test_backfill_overwrite_unclosed_with_closed` | 未闭合升级为已闭合 | 未闭合 K 线 + 已闭合回填 | `open=105.0`，`closed=true` | 确保已闭合数据优先 |
| 14 | `test_backfill_no_overwrite_closed_with_closed` | 已闭合不被覆盖 | 已闭合 K 线 + 不同的已闭合回填 | `open=100.0`（原始保留） | 永不覆盖已确认数据 |
| 15 | `test_backfill_no_overwrite_closed_with_unclosed` | 已闭合不被未闭合覆盖 | 已闭合 K 线 + 未闭合回填 | 原始保留 | 防止数据回退 |
| 16 | `test_capacity_enforcement_on_update` | 单次更新的最大容量 | `limit + 50` 次连续更新 | 数量 = `limit` | 连续更新保持有界 |
| 17 | `test_get_all_timeframes` | 所有周期快照 | M1 + M5 K 线 | `m1.len()=1`, `m5.len()=1`, `m15.len()=0` | 完整数据导出 |
| 18 | `test_get_klines_nonexistent_timeframe` | 空周期查询 | 空缓存 | 空数组 | 缺失数据的安全默认值 |
| 19 | `test_is_empty_after_data` | 非空检测 | 添加一根 K 线 | `is_empty() == false` | 状态追踪 |
| 20 | `test_multiple_timeframes_independent` | 周期隔离 | M1: 2 根，M5: 1 根 | `M1=2`, `M5=1`, `H1=0` | 周期之间互不干扰 |
| 21 | `test_backfill_truncation` | 回填容量限制 | `limit + 100` 根回填 | 数量 = `limit` | 回填也遵守容量限制 |
| 22 | `test_timeframe_buffer_push_or_update_order_preserved` | 插入顺序 | 3 根连续 K 线 | 顺序：0, 60000, 120000 | 时间顺序保持 |

## 3. aggregator.rs — 周期聚合（21 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_aggregate_1m_to_5m_basic` | 基础 5m 聚合 | 5 根已闭合 1m K 线 | 1 根已闭合 5m K 线 | 核心聚合逻辑 |
| 2 | `test_aggregate_1m_to_5m_partial` | 不完整的 5m 窗口 | 3 根已闭合 1m K 线 | 1 根未闭合 5m K 线 | 处理不完整窗口 |
| 3 | `test_aggregate_1m_to_5m_empty` | 空输入 | 无 K 线 | 空结果 | 优雅处理空输入 |
| 4 | `test_aggregate_1m_to_15m` | 15m 聚合 | 15 根已闭合 1m K 线 | 1 根已闭合 15m K 线 | 多步聚合 |
| 5 | `test_aggregate_1m_to_h1` | 1h 聚合 | 60 根已闭合 1m K 线 | 1 根已闭合 H1 K 线 | 小时级 K 线构建 |
| 6 | `test_aggregate_1m_to_h4` | 4h 聚合 | 240 根已闭合 1m K 线 | 1 根已闭合 H4 K 线 | 扩展周期 |
| 7 | `test_aggregate_1m_to_d1` | 1d 聚合 | 1440 根已闭合 1m K 线 | 1 根已闭合 D1 K 线 | 日 K 线构建 |
| 8 | `test_aggregate_5m_to_15m` | 跨周期聚合 | 3 根已闭合 5m K 线 | 1 根已闭合 15m K 线 | 从高级周期聚合更高级周期 |
| 9 | `test_aggregate_mixed_closed_unclosed` | 混合闭合状态 | 3 根已闭合 + 2 根未闭合 | 1 根未闭合 5m K 线 | 未闭合状态传播 |
| 10 | `test_aggregate_preserves_ohlcv` | OHLCV 正确性 | 已知价格的 K 线 | 正确的 OHLCV 聚合 | 数据完整性 |
| 11 | `test_aggregate_volume_sum` | 成交量求和 | 成交量 50.0 的 K 线 | 5m 成交量 = 250.0 | 成交量算术 |
| 12 | `test_aggregate_trades_sum` | 成交笔数求和 | 100 笔成交的 K 线 | 5m 成交笔数 = 500 | 成交计数 |
| 13 | `test_aggregate_high_low` | 最高/最低价极值 | 不同 H/L 的 K 线 | 最大 high，最小 low | 价格区间保持 |
| 14 | `test_aggregate_open_time_alignment` | 开盘时间对齐 | 各种时间的 1m K 线 | 对齐到 5m 边界 | 时间标准化 |
| 15 | `test_aggregate_multiple_windows` | 多聚合窗口 | 12 根已闭合 1m K 线 | 2 根已闭合 5m + 1 根未闭合 | 多窗口处理 |
| 16 | `test_aggregate_single_candle` | 单根 K 线边界 | 1 根已闭合 1m K 线 | 1 根未闭合 5m K 线 | 最小输入 |
| 17 | `test_aggregate_with_gap` | 数据缺口 | 2 根间隔 10 分钟的 K 线 | 2 个独立窗口 | 缺口处理 |
| 18 | `test_aggregate_close_time` | 收盘时间正确性 | 5 根已闭合 1m K 线 | `close_time = open_time + 299999` | 正确的区间结束 |
| 19 | `test_aggregate_empty_timeframe` | 空周期结果 | 无匹配 K 线 | 空数组 | 无假阳性 |
| 20 | `test_update_higher_timeframes` | 完整高级周期更新 | 5 根已闭合 1m K 线 | M5 已更新，其余为空 | 与缓存的集成 |
| 21 | `test_update_higher_timeframes_multiple` | 多高级周期更新 | 60 根已闭合 1m K 线 | M5, M15, H1 全部更新 | 级联聚合 |

## 4. gap.rs — 缺口检测与回填（11 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_initial_load_basic` | 初始数据加载 | 2000 根 1m + 1000 根高级周期 K 线 | `result > 0`，M1 数量 > 0 | 完整初始加载路径 |
| 2 | `test_initial_load_no_1m_data` | 无 1m 数据 | 空 mock 数据源 | `result == 0` | 优雅处理空数据源 |
| 3 | `test_initial_load_1m_error` | 1m 获取失败 | Mock 返回错误 | `result.is_err()` | 错误传播 |
| 4 | `test_initial_load_high_tf_partial_failure` | 高级周期部分失败 | 部分周期返回错误 | `result > 0`（部分成功） | 对部分失败的容错性 |
| 5 | `test_no_gap_when_up_to_date` | 无缺口场景 | 缓存中有最近 K 线 | `result == 0` | 跳过不必要的回填 |
| 6 | `test_gap_backfill` | 缺口检测和填充 | 旧 K 线 + 缺口数据 | `result > 0` | 缺口填充功能 |
| 7 | `test_gap_backfill_empty_response` | 缺口但无数据源 | 旧 K 线，空数据源 | `result == 0` | 处理缺失的缺口数据 |
| 8 | `test_check_continuity_empty_cache` | 空缓存连续性检查 | 空缓存 | `is_continuous=false`，`missing_minutes=MAX` | 检测缺失数据 |
| 9 | `test_check_continuity_up_to_date` | 数据最新时的连续性 | 最近的已闭合 K 线 | `is_continuous=true`，`missing_minutes=0` | 确认数据新鲜度 |
| 10 | `test_check_continuity_gap_detected` | 缺口检测 | 60 分钟前的 K 线 | `is_continuous=false`，`missing_minutes > 0` | 识别数据缺口 |
| 11 | `test_initial_load_event_broadcast` | 初始加载事件广播 | 100 根 1m K 线 | 收到 `Backfilled` 事件 | 通知订阅者数据加载 |

## 5. mod.rs — KlineEngine 集成测试（20 个测试）

### 5.1 订阅管理（7 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_subscribe_creates_entry` | 基础订阅 | `subscribe("binance", "BTCUSDT", Spot)` | `is_subscribed("binance", "BTCUSDT") == true` | 验证订阅创建内部条目 |
| 2 | `test_subscribe_idempotent` | 重复订阅 | 连续两次 `subscribe("binance", "BTCUSDT", Spot)` | `subscribed_symbols().len() == 1` | 防止重复订阅 |
| 3 | `test_subscribe_multiple_symbols` | 多交易对订阅 | `subscribe("binance", "BTCUSDT", Spot)` + `subscribe("binance", "ETHUSDT", Spot)` | `len() == 2`，两者均 `is_subscribed` | 独立订阅追踪 |
| 4 | `test_subscribe_spot_calls_spot_ws` | 现货 WS 委托 | `subscribe("binance", "BTCUSDT", Spot)` | `spot_ws.subscribed_symbols` 包含 `"BTCUSDT"` | 现货市场路由到现货 WS 客户端 |
| 5 | `test_subscribe_perpetual_calls_perpetual_ws` | 永续 WS 委托 | `subscribe("binance", "BTCUSDT", Perpetual)` | `perpetual_ws.subscribed_symbols` 包含 `"BTCUSDT"` | 永续市场路由到永续 WS 客户端 |
| 6 | `test_unsubscribe` | 基础取消订阅 | `subscribe` 后 `unsubscribe("binance", "BTCUSDT")` | `is_subscribed == false` | 清理订阅条目 |
| 7 | `test_unsubscribe_nonexistent` | 取消不存在的订阅 | `unsubscribe("binance", "NONEXISTENT")` | 返回 `Ok(())` | 优雅处理无效取消 |

### 5.2 数据查询（4 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 8 | `test_get_klines_unsubscribed` | 查询未订阅的交易对 | 未订阅时 `get_klines("binance", "BTCUSDT", H1)` | `None` | 防止无效查询 |
| 9 | `test_get_klines_async_unsubscribed` | 异步查询未订阅的交易对 | 未订阅时 `get_klines_async("binance", "BTCUSDT", H1)` | `None` | 异步变体返回相同结果 |
| 10 | `test_get_klines_after_subscribe_with_backfill` | 订阅后回填数据 | `subscribe`（`backfill_on_start=true`），Mock 提供 10 根 1m K 线 | `Some`，`len() > 0` | 验证数据从数据源流经缓存 |
| 11 | `test_get_all_timeframes` | 全周期数据 | `subscribe`（回填开启），Mock 提供数据 | `Some` | 完整周期快照可用 |

### 5.3 事件系统（3 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 12 | `test_subscribe_events` | 事件接收器创建 | `subscribe_events()` | 返回 `Receiver`，无 panic | 广播通道初始化 |
| 13 | `test_subscribe_events_multiple_receivers` | 多接收器 | 连续两次 `subscribe_events()` | 两个接收器均创建成功 | 扇出能力 |
| 14 | `test_event_broadcast_on_backfill` | 回填事件广播 | `subscribe`（`backfill_on_start=true`），Mock 提供 5 根 1m K 线 | `event_rx` 收到 `Backfilled` 事件 | 通知订阅者数据加载 |

### 5.4 回测工具（4 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 15 | `test_backtest_range_limits` | 范围限制枚举 | `backtest_range_limits()` | 返回 6 个限制配置 | 所有周期完整覆盖 |
| 16 | `test_validate_backtest_range_valid` | 有效范围验证 | `validate_backtest_range(M1, 3)` | `Ok(())` | 在推荐范围内 |
| 17 | `test_validate_backtest_range_exceeds_max` | 超出最大值验证 | `validate_backtest_range(M1, 100)` | `Err` | 防止过多数据请求 |
| 18 | `test_validate_backtest_range_recommended_warning` | 警告范围验证 | `validate_backtest_range(M1, 5)`（推荐=3，最大=7） | `Ok(())` 并打印警告日志 | 大范围的提示 |

### 5.5 连续性/回填（2 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 19 | `test_continuity_check_unsubscribed` | 未订阅时连续性检查 | 未订阅时 `continuity_check("binance", "BTCUSDT")` | `None` | 防止无效连续性检查 |
| 20 | `test_force_backfill_unsubscribed` | 未订阅时强制回填 | 未订阅时 `force_backfill("binance", "BTCUSDT")` | `Err` | 防止对未知交易对回填 |

## 6. binance_kline_ws.rs — 交易所 WebSocket 客户端（15 个测试）

### 6.1 消息解析（5 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_parse_binance_kline_message` | 标准 kline 消息解析 | Binance WS JSON：`{"stream":"btcusdt@kline_1m","data":{"e":"kline","E":1713900000,"s":"BTCUSDT","k":{"t":1713900000000,"T":1713900059999,"s":"BTCUSDT","i":"1m","o":"65000.00","h":"65100.00","l":"64900.00","c":"65050.00","v":"100.5","n":500,"x":false,"q":"6532500.00"}}}` | `stream == Some("btcusdt@kline_1m")`，`data != None` | 验证标准 Binance WS 消息格式解析 |
| 2 | `test_parse_binance_kline_closed` | 已闭合 K 线消息 | 同上但 `"x": true` | `data.kline.closed == true` | 验证 closed 标志正确解析 |
| 3 | `test_parse_binance_kline_message_without_stream` | 无 stream 字段 | `{"data":{"e":"kline",...}}` | `stream == None`，`data != None` | 兼容无 stream 的消息格式 |
| 4 | `test_parse_invalid_json` | 无效 JSON | `"not json"` | 反序列化返回 `Err` | 防御无效输入 |
| 5 | `test_parse_non_kline_event` | 非 kline 事件类型 | `{"data":{"e":"trade",...}}` | 反序列化返回 `Err`（缺少 k 字段） | 非 kline 事件不应被处理 |

### 6.2 Candle 转换（4 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 6 | `test_to_candle_basic` | 基础 K 线转换 | `BinanceKlineData`：`t=1713900000000, T=1713900059999, o="65000.00", h="65100.00", l="64900.00", c="65050.00", v="100.5", n=500, x=false, q="6532500.00"` | `Candle { open_time: 1713900000000, close_time: 1713900059999, open: 65000.0, high: 65100.0, low: 64900.0, close: 65050.0, volume: 100.5, quote_volume: 6532500.0, trades: 500, closed: false }` | 验证字段一一对应转换 |
| 7 | `test_to_candle_closed` | 已闭合 K 线转换 | 同上但 `x=true` | `candle.closed == true` | closed 标志正确传递 |
| 8 | `test_to_candle_invalid_numbers` | 无效数字字段 | `o="not_a_number", h="abc"` | `open: 0.0, high: 0.0` | 解析失败回退到 0.0 |
| 9 | `test_ws_symbol` | WS symbol 提取 | `k.s = "BTCUSDT"` | `ws_symbol() == "BTCUSDT"` | 正确提取交易对 |

### 6.3 Symbol 转换（3 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 10 | `test_binance_ws_symbol_basic` | 基础转换 | `"BTCUSDT"` | `"btcusdt"` | 小写化 |
| 11 | `test_binance_ws_symbol_with_slash` | 带斜杠转换 | `"BTC/USDT"` | `"btcusdt"` | 去除斜杠 + 小写化 |
| 12 | `test_binance_ws_symbol_lowercase` | 已小写输入 | `"btcusdt"` | `"btcusdt"` | 幂等性 |

### 6.4 构造函数和状态（3 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 13 | `test_new_spot` | 现货 WS 构造 | `BinanceKlineWs::new_spot(None)` | `ws_url == "wss://stream.binance.com/ws"`，`is_running() == false` | 验证现货 WS URL |
| 14 | `test_new_perpetual` | 永续 WS 构造 | `BinanceKlineWs::new_perpetual(None)` | `ws_url == "wss://fstream.binance.com/ws"`，`is_running() == false` | 验证永续 WS URL |
| 15 | `test_subscribe_without_start` | 未启动时订阅 | 不调用 `start()`，直接 `subscribe("BTCUSDT")` | `subscriptions` 包含 `"btcusdt@kline_1m"`，`symbol_map` 包含映射，无 panic | 验证未连接时订阅不崩溃 |

## 7. api/kline.rs — 前端 WebSocket Handler（10 个测试）

### 7.1 事件序列化（3 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 1 | `test_kline_event_serialization` | Update 事件序列化 | `KlineEvent { exchange: "binance", symbol: "BTCUSDT", timeframe: H1, candle: ..., event_type: Update }` | JSON 包含 `"exchange":"binance"` 和 `"symbol":"BTCUSDT"` | 验证前端 WS 推送格式 |
| 2 | `test_kline_event_closed_serialization` | Closed 事件序列化 | `KlineEvent { ..., event_type: Closed }` | JSON 包含 `"event_type":"Closed"` | 已闭合事件正确序列化 |
| 3 | `test_kline_event_backfilled_serialization` | Backfilled 事件序列化 | `KlineEvent { ..., event_type: Backfilled }` | JSON 包含 `"event_type":"Backfilled"` | 回填事件正确序列化 |

### 7.2 Broadcast Channel 行为（4 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 4 | `test_broadcast_lagged_recovery` | Lagged 后恢复 | `channel(capacity=2)`，发送 5 条消息 | 第一次 `recv()` 返回 `Err(Lagged(3))`，第二次返回 `Ok(event)` | 验证 Lagged 不导致永久停止 |
| 5 | `test_broadcast_lagged_then_closed` | Lagged 后 Channel 关闭 | receiver lagged 后 `drop(sender)` | 先收到 `Lagged`，drain 完后收到 `Closed` | 验证关闭检测 |
| 6 | `test_broadcast_multiple_receivers` | 多接收器 | 1 个 sender，3 个 receiver | 发送 1 条消息，3 个 receiver 都收到 | 验证扇出能力 |
| 7 | `test_broadcast_receiver_lagged_independently` | 接收器独立 Lagged | 2 个 receiver 都不消费，发送 5 条 | 两个 receiver 都收到 `Lagged`，恢复后都能正常接收 | 验证接收器互不影响 |

### 7.3 API 响应序列化（3 个测试）

| # | 测试名 | 场景 | 输入 | 期望输出 | 测试理由 |
|---|--------|------|------|----------|----------|
| 8 | `test_kline_response_serialization` | 成功响应序列化 | `KlineResponse { success: true, data: SingleTimeframe([candle]) }` | JSON 序列化成功 | 验证 HTTP API 响应格式 |
| 9 | `test_kline_response_error` | 错误响应序列化 | `KlineResponse { success: false, error: "not subscribed" }` | JSON 包含 `"success":false` 和 `"error":"not subscribed"` | 验证错误响应格式 |
| 10 | `test_all_timeframes_response` | 全周期响应序列化 | `AllTimeframesData { m1: [candle], m5: [], ... }` | 通过 `KlineData::AllTimeframes` 序列化成功 | 验证多周期数据格式 |

---

## 汇总

| 模块 | 测试数 | 状态 |
|------|--------|------|
| types.rs | 18 | ✅ 全部通过 |
| cache.rs | 22 | ✅ 全部通过 |
| aggregator.rs | 21 | ✅ 全部通过 |
| gap.rs | 11 | ✅ 全部通过 |
| mod.rs | 20 | ✅ 全部通过 |
| binance_kline_ws.rs | 15 | ✅ 全部通过 |
| api/kline.rs | 10 | ✅ 全部通过 |
| **合计** | **117** | **全部通过** |
