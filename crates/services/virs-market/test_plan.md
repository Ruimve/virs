# virs-market - 测试计划

**测试总数:** 39（单元测试: 35，集成测试: 4）

---

## 单元测试

### `crates/services/virs-market/src/aggregator_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `a1_1_candle_from_1m_basic` | 1m 蜡烛转 M5 基本字段 | 1m 蜡烛: open=100.0, high=105.0, low=95.0, close=102.0, volume=100.0, trades=10 | open=100.0, high=105.0, low=95.0, close=102.0, volume=100.0, trades=10 |
| 2 | `a1_2_candle_from_1m_align` | 1m 转换后时间对齐到 M5 | 1m 蜡烛 open_time=BASE, 转为 M5 | open_time=align_open_time(BASE, M5), close_time=expected_open + M5.ms() - 1 |
| 3 | `a1_3_candle_from_1m_closed_false` | 1m 转换后 closed 标志为 false | 1m 蜡烛 closed=true, 转为 M5 | result.closed=false |
| 4 | `a2_1_is_last_1m_in_group_m5` | M5 组中第 5 根 1m 是最后一根 | M5 组第 5 根（偏移 4 分钟）1m 蜡烛 | is_last_1m_in_group=true |
| 5 | `a2_2_is_last_1m_not_last` | M5 组中第 3 根不是最后一根 | M5 组第 3 根（偏移 2 分钟）1m 蜡烛 | is_last_1m_in_group=false |
| 6 | `a2_3_is_last_1m_in_group_h1` | H1 组中第 60 根 1m 是最后一根 | H1 组第 60 根（偏移 59 分钟）1m 蜡烛 | is_last_1m_in_group=true |
| 7 | `a2_4_is_last_1m_exact_boundary` | M5 组首根不是最后一根 | M5 组首根（偏移 0）1m 蜡烛 | is_last_1m_in_group=false |
| 8 | `a3_1_aggregate_empty` | 空输入聚合 | 空蜡烛数组, 时间周期 M5 | 结果为空 |
| 9 | `a3_2_aggregate_single_candle` | 单根蜡烛聚合 | 1 根 1m 蜡烛, 时间周期 M5 | 结果长度=1, open=100.0, close=102.0 |
| 10 | `a3_3_aggregate_m5_full` | M5 完整 5 根聚合 | 5 根 1m 蜡烛, 价格 100~104, 时间周期 M5 | 结果长度=1, open=100.0, close=105.0, high=106.0, low=98.0, closed=true, volume=500.0 |
| 11 | `a3_4_aggregate_m5_partial` | M5 部分 3 根聚合 | 3 根 1m 蜡烛, 时间周期 M5 | 结果长度=1, closed=false |
| 12 | `a3_5_aggregate_multi_group` | 跨多组聚合 | 7 根 1m 蜡烛, 时间周期 M5 | 结果长度=2, 第一组 closed=true, 第二组 closed=false |
| 13 | `a4_1_align_then_aggregate` | 对齐后聚合时间正确 | 5 根 1m 蜡烛, 时间周期 M5 | 结果长度=1, open_time=start, close_time=start + M5.ms() - 1 |
| 14 | `a4_2_candle_from_1m_then_aggregate` | candle_from_1m 与聚合结果一致 | 1 根 1m 蜡烛, 时间周期 M5 | 聚合长度=1, open_time/open/close 与 candle_from_1m 一致 |
| 15 | `a4_3_aggregate_then_cache_update` | 聚合后写入缓存 | 5 根 1m 聚合为 1 根 M5, 写入 SymbolCache | klines 长度=1, open_time=start |
| 16 | `a4_4_aggregate_with_gap` | 含时间间隔的聚合 | 2 根 1m 蜡烛（跳过第 2 根）, 时间周期 M5 | 聚合长度=1, volume=200.0 |
| 17 | `a4_5_aggregate_full_day_to_d1` | 完整一天 1440 根聚合为 D1 | 1440 根 1m 蜡烛, 价格 100.0 起, 时间周期 D1 | 结果长度=1, open=100.0, closed=true, volume=144000.0 |

### `crates/services/virs-market/src/types_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `t1_1_timeframe_all_count` | Timeframe 全部变体数量 | Timeframe::all() | 长度=6 |
| 2 | `t1_2_timeframe_as_str` | Timeframe 字符串表示 | M1, M5, M15, H1, H4, D1 | "1m", "5m", "15m", "1h", "4h", "1d" |
| 3 | `t1_3_timeframe_ms` | Timeframe 毫秒值 | M1, M5, M15, H1, H4, D1 | 60000, 300000, 900000, 3600000, 14400000, 86400000 |
| 4 | `t1_5_timeframe_default_limit` | Timeframe 默认限制数 | 所有 Timeframe 变体 | default_limit()=1000 |
| 5 | `t1_7_timeframe_display` | Timeframe Display 格式化 | M1, H1, D1 | "1m", "1h", "1d" |
| 6 | `t2_1_subscription_key_basic` | 订阅键基本格式 | exchange=binance, symbol=BTCUSDT | "binance:BTCUSDT" |
| 7 | `t2_2_subscription_key_case` | 订阅键大小写归一化 | exchange=Binance, symbol=btc/usdt | "binance:BTCUSDT" |
| 8 | `t3_1_align_open_time_m1` | M1 时间对齐 | 时间戳 1700000000500 | 对齐到 60_000 的整数倍 |
| 9 | `t3_2_align_open_time_h1` | H1 时间对齐 | 时间戳 1700001234567 | 对齐值 % 3_600_000=0 |
| 10 | `t3_3_align_open_time_d1` | D1 时间对齐 | 时间戳 1700001234567 | 对齐值 % 86_400_000=0 |
| 11 | `t3_4_align_open_time_exact` | 整数倍时间对齐不变 | 时间戳 1_700_000_040_000（已是 60_000 倍数） | 对齐值=原值 |

### `crates/services/virs-market/src/cache_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `c1_1_update_and_get` | 更新并获取 K 线 | 1 根 M1 蜡烛 close=100.0, closed=false | klines 长度=1, open_time 与输入一致, close=100.0 |
| 2 | `c1_2_update_same_open_time` | 相同 open_time 更新覆盖 | 两根相同 open_time 蜡烛 close=100.0 和 105.0 | klines 长度=1, close=105.0 |
| 3 | `c1_3_update_old_candle` | 更新旧蜡烛保留历史 | 先更新较新蜡烛后更新较旧蜡烛 | klines 长度=2, 包含旧蜡烛 open_time |
| 4 | `c1_4_max_size_eviction` | 超过最大容量淘汰 | 插入 1005 根 M1 蜡烛 | klines 长度=1000, 首根 open_time=5*60_000 |
| 5 | `c2_1_close_candle` | 关闭指定蜡烛 | 1 根未关闭 M1 蜡烛, 调用 close_candle | klines[0].closed=true |
| 6 | `c2_2_last_closed_1m` | 获取最后一根已关闭 1m | 1 根已关闭 + 1 根未关闭 M1 蜡烛 | last_closed_1m 返回已关闭蜡烛, open_time=1_700_000_000_000 |
| 7 | `c3_1_replace_timeframe` | 替换整个时间周期数据 | 替换为 5 根 close=100~104 的 M1 蜡烛 | klines 长度=5, 首根 open_time=0, 末根 close=104.0 |

## 集成测试

### `crates/services/virs-market/tests/integration_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `int_2_1_candle_from_1m_basic` | 1m 转 M5 基本集成 | 1m 蜡烛: open=100.0, high=102.0, low=98.0, close=101.0, 转为 M5 | open_time % M5.ms()=0, open=100.0, close=101.0 |
| 2 | `int_3_1_subscription_key_then_check` | 订阅键生成与一致性 | exchange=binance, symbol=BTCUSDT（两次调用） | 两次结果相等, 含 ':', 以 "binance:" 开头 |
| 3 | `int_3_2_align_multi_timeframe` | 多周期时间对齐层级关系 | 时间戳 BASE+123456, 对齐到 M1/M5/H1/D1 | d1<=h1<=m5<=m1, 各周期对齐值取模为 0 |
| 4 | `int_6_1_timeframe_str_to_ms` | 时间周期字符串转毫秒 | "1m","5m","15m","1h","4h","1d","invalid" | 60000, 300000, 900000, 3600000, 14400000, 86400000, 无效输入返回 60000 |
