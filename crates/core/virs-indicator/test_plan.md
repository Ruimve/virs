# virs-indicator - 测试计划

**测试总数:** 91（单元测试: 91，集成测试: 0）

---

## 单元测试

### `crates/core/virs-indicator/src/indicators/atomic/adx_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `adx_higher_in_trend_than_sideways` | 趋势行情下 ADX 高于震荡行情 | 趋势: uptrend_klines(60,100,2.0), 震荡: sideways_klines(60,100,0.5), 周期14, index=59 | trend_adx > sideways_adx |
| 2 | `adx_positive` | 上涨趋势中 ADX 为正值 | uptrend_klines(60,100,1.0), 周期14, index=59 | val > 0.0 |
| 3 | `adx_errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(20,100,1.0), 周期14, index=19 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/atomic/atr_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `atr_positive_for_sufficient_data` | 数据充足时 ATR 为正值 | uptrend_klines(60,100,1.0), 周期14, index=last | val > 0.0 |
| 2 | `atr_series_correct_length` | ATR 序列长度与 K 线数量一致 | uptrend_klines(60,100,1.0), 周期14 | series.len() == 60 |
| 3 | `atr_zero_for_zero_range_klines` | 零波幅 K 线的 ATR 约为 0 | 30 条 kline(100,100,100,100,1000), 周期14, index=last | |val| < 0.001 |
| 4 | `atr_errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), 周期14, index=9 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/atomic/bbands_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `bbands_upper_above_middle_above_lower` | 布林带上下轨顺序正确（上>中>下） | uptrend_klines(60,100,1.0), 周期20, 标准差2.0, index=last | upper > middle > lower |
| 2 | `bbands_middle_near_last_price_in_sideways` | 震荡行情中布林带中轨接近最新价 | sideways_klines(60,100,2.0), 周期20, 标准差2.0, index=last | |middle - price| < 5.0 |
| 3 | `bbands_width_positive` | 布林带带宽为正 | uptrend_klines(60,100,1.0), 周期20, 标准差2.0, index=last | upper - lower > 0.0 |
| 4 | `bbands_errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), 周期20, 标准差2.0, index=9 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/atomic/ema_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `ema_returns_value_for_sufficient_data` | 数据充足时 EMA 返回有效值 | uptrend_klines(60,100,1.0), 周期20, index=last | val > 100.0 |
| 2 | `ema_lagging_behind_price_in_uptrend` | 上涨趋势中 EMA 滞后于价格 | uptrend_klines(60,100,1.0), 周期20, index=last | ema < price |
| 3 | `ema_shorter_period_closer_to_price` | 短周期 EMA 比长周期更接近价格 | uptrend_klines(60,100,1.0), 周期20与50, index=last | |price-ema20| < |price-ema50| |
| 4 | `ema_errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), 周期20, index=9 | 返回错误 |
| 5 | `ema_errors_on_empty_klines` | 空 K 线列表返回错误 | 空列表, 周期20, index=0 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/atomic/highest_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `highest_returns_max_high` | 返回窗口内最高价的最大值 | uptrend_klines(30,100,1.0), 周期20, index=last | val == 窗口内最大 high |
| 2 | `highest_of_constant_is_constant` | 常量 K 线的最高价为常量 | 30 条 kline(100,105,95,100,1000), 周期20, index=29 | val == 105.0 |
| 3 | `highest_errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), 周期20, index=9 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/atomic/lowest_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `lowest_returns_min_low` | 返回窗口内最低价的最小值 | uptrend_klines(30,100,1.0), 周期20, index=last | val == 窗口内最小 low |
| 2 | `lowest_of_constant_is_constant` | 常量 K 线的最低价为常量 | 30 条 kline(100,105,95,100,1000), 周期20, index=29 | val == 95.0 |
| 3 | `lowest_errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), 周期20, index=9 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/atomic/macd_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `macd_positive_in_uptrend` | 上涨趋势中 MACD 为正 | uptrend_klines(60,100,1.0), fast=12, slow=26, index=last | val > 0.0 |
| 2 | `macd_negative_in_downtrend` | 下跌趋势中 MACD 为负 | downtrend_klines(60,200,1.0), fast=12, slow=26, index=last | val < 0.0 |
| 3 | `macd_signal_exists_for_sufficient_data` | 数据充足时信号线为有限值 | uptrend_klines(60,100,1.0), fast=12, slow=26, signal=9, index=last | sig.is_finite() == true |
| 4 | `macd_errors_on_insufficient_data` | MACD 数据不足时返回错误 | uptrend_klines(20,100,1.0), fast=12, slow=26, index=19 | 返回错误 |
| 5 | `macd_signal_errors_on_insufficient_data` | 信号线数据不足时返回错误 | uptrend_klines(30,100,1.0), fast=12, slow=26, signal=9, index=29 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/atomic/rsi_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `rsi_above_50_in_uptrend` | 上涨趋势中 RSI 大于 50 | uptrend_klines(60,100,1.0), 周期14, index=last | rsi > 50.0 |
| 2 | `rsi_below_50_in_downtrend` | 下跌趋势中 RSI 小于 50 | downtrend_klines(60,200,1.0), 周期14, index=last | rsi < 50.0 |
| 3 | `rsi_near_50_in_sideways` | 震荡行情中 RSI 接近 50 | sideways_klines(60,100,1.0), 周期14, index=last | |rsi - 50.0| < 20.0 |
| 4 | `rsi_errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), 周期14, index=9 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/atomic/sma_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `sma_of_constant_series` | 常量序列的 SMA 等于常量值 | series=[100.0;20], 周期10, index=19 | val == 100.0 |
| 2 | `sma_of_increasing_series` | 递增序列的 SMA 计算正确 | series=0..20, 周期5, index=19 | val == 17.0 |
| 3 | `sma_handles_nan_values` | SMA 正确过滤 NaN 值 | series=[100.0;20] 且 index5=NaN, 周期5, index=19 | val.is_finite() == true |
| 4 | `sma_errors_on_empty_series` | 空序列返回错误 | series=[], 周期10, index=0 | 返回错误 |
| 5 | `sma_errors_on_zero_period` | 零周期返回错误 | series=[100.0;10], 周期0, index=5 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/atomic/volume_sma_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `volume_sma_of_constant_volume` | 常量成交量的成交量 SMA 等于常量值 | 30 条 kline(100,105,95,100,1000), 周期5, index=28 | val == 1000.0 |
| 2 | `volume_sma_of_increasing_volume` | 递增成交量的成交量 SMA 计算正确 | uptrend_klines(30,100,1.0), 周期5, index=28 | val == (1240+1250+1260+1270+1280)/5 |
| 3 | `volume_sma_errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(5,100,1.0), 周期10, index=3 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/primitive/candle_body_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `positive_for_bullish_candle` | 阳线实体为正 | kline(100,105,95,103,1000) | val == 3.0 |
| 2 | `negative_for_bearish_candle` | 阴线实体为负 | kline(103,105,95,100,1000) | val == -3.0 |
| 3 | `zero_for_doji` | 十字星实体约为 0 | kline(100,105,95,100,1000) | |val| < 0.001 |
| 4 | `errors_on_empty_klines` | 空 K 线列表返回错误 | 空列表 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/primitive/change_pct_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `positive_in_uptrend` | 上涨趋势中涨跌幅为正 | uptrend_klines(30,100,1.0), period=1 | val > 0.0 |
| 2 | `correct_value_for_1_period` | 1 周期涨跌幅计算正确 | uptrend_klines(30,100,1.0), period=1 | val == (129-128)/128*100 |
| 3 | `zero_change_for_constant_price` | 常量价格涨跌幅约为 0 | 30 条 kline(100,105,95,100,1000), period=1 | |val| < 0.001 |
| 4 | `errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(5,100,1.0), period=10 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/primitive/current_price_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `returns_last_close` | 返回最后一根 K 线的收盘价 | uptrend_klines(30,100,1.0) | val == 最后 K 线的 close |
| 2 | `returns_single_kline_close` | 单根 K 线返回其收盘价 | kline(100,105,95,103,1000) | val == 103.0 |
| 3 | `errors_on_empty_klines` | 空 K 线列表返回错误 | 空列表 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/primitive/last_volume_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `returns_second_to_last_volume` | 返回倒数第二根 K 线的成交量 | uptrend_klines(30,100,1.0) | val == klines[len-2].volume |
| 2 | `returns_correct_volume_for_manual_klines` | 手动构造 K 线返回正确的成交量 | 3 条 kline, 成交量 500/800/1200 | val == 800.0 |
| 3 | `errors_on_empty_klines` | 空 K 线列表返回错误 | 空列表 | 返回错误 |
| 4 | `errors_on_single_kline` | 单根 K 线返回错误 | 1 条 kline | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/atr_pct_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `atr_pct_positive` | ATR 百分比为正 | uptrend_klines(60,100,1.0), 周期14 | val > 0.0 |
| 2 | `atr_pct_near_zero_for_flat_klines` | 平盘 K 线的 ATR 百分比约为 0 | 30 条 kline(100,100,100,100,1000), 周期14 | |val| < 0.01 |
| 3 | `errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(5,100,1.0), 周期14 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/atr_sma_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `atr_sma_positive` | ATR 的 SMA 为正 | uptrend_klines(60,100,1.0), atr周期14, sma周期20 | val > 0.0 |
| 2 | `atr_sma_near_zero_for_flat_klines` | 平盘 K 线的 ATR SMA 约为 0 | 40 条 kline(100,100,100,100,1000), atr周期14, sma周期20 | |val| < 0.01 |
| 3 | `errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), atr周期14, sma周期20 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/bandwidth_bars_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `returns_valid_width_5_bars_ago` | 5 根 K 线前的带宽为正 | uptrend_klines(60,100,1.0), 周期20, 标准差2, bars_ago=5 | val > 0.0 |
| 2 | `errors_when_bars_ago_exceeds_data` | bars_ago 超过数据长度时返回错误 | uptrend_klines(30,100,1.0), bars_ago=50 | 返回错误 |
| 3 | `errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), bars_ago=5 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/bars_outside_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `zero_when_price_inside_band` | 价格在布林带内时返回 0 | 30 条 kline(100,101,99,100,1000), 周期20, 标准差2 | val == 0 |
| 2 | `positive_when_price_above_upper` | 价格突破上轨时返回正值 | 25 条平盘 + 5 条上涨突破 K 线, 周期20, 标准差2 | val > 0 |
| 3 | `errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), 周期20, 标准差2 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/bbands_width_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `width_positive_for_sufficient_data` | 数据充足时布林带带宽为正 | uptrend_klines(60,100,1.0), 周期20, 标准差2, index=last | val > 0.0 |
| 2 | `width_smaller_in_sideways_than_trend` | 震荡行情带宽小于趋势行情 | 趋势: uptrend_klines(60,100,2.0), 震荡: sideways_klines(60,100,0.5), 周期20, 标准差2 | sideways_width < trend_width |
| 3 | `width_errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), 周期20, 标准差2, index=9 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/ema_cross_bars_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `returns_minus_one_for_no_cross_in_uptrend` | 稳定上涨无交叉时返回 -1 | uptrend_klines(100,100,1.0), 周期20,50 | val == -1 |
| 2 | `returns_non_negative_when_cross_exists` | 存在交叉时返回非负值 | 50 条平盘 + 10 条下跌 + 15 条上涨 K 线, 周期20,50 | val >= 0 |
| 3 | `errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(10,100,1.0), 周期20,50 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/ema_cross_state_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `golden_cross_in_uptrend` | 上涨趋势中为金叉（多头） | uptrend_klines(60,100,1.0), 周期20,50 | val == "金叉(多头)" |
| 2 | `death_cross_in_downtrend` | 下跌趋势中为死叉（空头） | downtrend_klines(60,200,1.0), 周期20,50 | val == "死叉(空头)" |
| 3 | `errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(30,100,1.0), 周期20,50 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/ema_gap_pct_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `positive_in_uptrend` | 上涨趋势中 EMA 间距百分比为正 | uptrend_klines(60,100,1.0), 周期20,50 | val > 0.0 |
| 2 | `negative_in_downtrend` | 下跌趋势中 EMA 间距百分比为负 | downtrend_klines(60,200,1.0), 周期20,50 | val < 0.0 |
| 3 | `errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(30,100,1.0), 周期20,50 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/ema_gap_trend_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `returns_valid_string` | 返回有效的趋势字符串 | uptrend_klines(60,100,1.0), 周期20,50 | val 为 "扩大"/"缩小"/"持平" 之一 |
| 2 | `expanding_in_accelerating_uptrend` | 加速上涨中间距扩大 | 60 条加速上涨 K 线 (close=100+i^2*0.05), 周期20,50 | val == "扩大" |
| 3 | `errors_on_insufficient_data` | 数据不足时返回错误 | uptrend_klines(30,100,1.0), 周期20,50 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/macd_histogram_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `histogram_positive_in_accelerating_uptrend` | 加速上涨中 MACD 柱状图为正 | 60 条加速上涨 K 线 (close=100+i^2*0.1), fast=12, slow=26, signal=9 | val > 0.0 |
| 2 | `histogram_negative_in_accelerating_downtrend` | 加速下跌中 MACD 柱状图为负 | 60 条加速下跌 K 线 (close=200-i^2*0.1), fast=12, slow=26, signal=9 | val < 0.0 |
| 3 | `histogram_errors_on_insufficient_data` | 数据不足时返回错误 | 20 条 K 线, fast=12, slow=26, signal=9 | 返回错误 |

### `crates/core/virs-indicator/src/indicators/derived/round_number_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `up_rounds_to_next_hundred` | 向上取整到下一个百位 | price=4567.0 | val == 4600.0 |
| 2 | `down_rounds_to_prev_hundred` | 向下取整到上一个百位 | price=4567.0 | val == 4500.0 |
| 3 | `up_rounds_small_price` | 小价格向上取整到十位 | price=123.0 | val == 130.0 |
| 4 | `down_rounds_small_price` | 小价格向下取整到十位 | price=123.0 | val == 120.0 |
| 5 | `already_at_round_number_stays` | 已是整数价格时保持不变 | price=100.0 | up==100.0, down==100.0 |
| 6 | `zero_price_returns_zero` | 零价格返回零 | price=0.0 | up==0.0, down==0.0 |
| 7 | `large_price_uses_thousands_step` | 大价格使用千位步长 | price=56789.0 | val == 57000.0 |

## 集成测试

*本 crate 无集成测试目录（tests/）。*
