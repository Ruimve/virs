当前时间：{timestamp}

## 账户资产
- 总资产：{total_balance} USDT
- 可用余额：{available_balance} USDT
- 已用保证金：{used_margin} USDT
- 保证金使用率：{margin_usage_rate}%

## 交易对与市场
- 交易对：{symbol}
- 交易所：{exchange}
- 杠杆倍数：{leverage}
- 最小交易数量：{min_qty}

## 当前仓位
{position_info}

## 持仓时长
{position_duration}

## 止损止盈（代码自动管理）
{stop_take_profit_info}

## 最近平仓事件（用于反思，避免反复扫损）
{recent_close_info}

## 资金费率（合约）
- 当前资金费率：{funding_rate}
- 下一个结算时间：{funding_next_time}

## 4小时大环境（背景参考，非硬性过滤）
- EMA20：{h4_ema20}
- EMA50：{h4_ema50}
- ADX(14)：{h4_adx}
- RSI(14)：{h4_rsi}
- MACD柱状图：{h4_macd_histogram}

## 1小时趋势过滤
- EMA20：{h1_ema20}
- EMA50：{h1_ema50}
- EMA交叉状态：{h1_ema_cross}
- RSI(14)：{h1_rsi}
- MACD柱状图：{h1_macd_histogram}
- ADX(14)：{h1_adx}

## 15分钟主信号周期（核心决策依据）
- 当前价格：{m15_current_price}
- EMA20：{m15_ema20}
- EMA50：{m15_ema50}
- EMA交叉状态：{m15_ema_cross}
- EMA间距：{m15_ema_gap_pct}%
- RSI(14)：{m15_rsi}
- MACD：{m15_macd}
- MACD信号线：{m15_macd_signal}
- MACD柱状图：{m15_macd_histogram}
- ADX(14)：{m15_adx}
- ATR(14)：{m15_atr}
- ATR(14)的20周期SMA：{m15_atr_sma20}
- 布林带上/中/下轨：{m15_bb_upper} / {m15_bb_middle} / {m15_bb_lower}
- 布林带宽度：{m15_bb_width_pct}%
- 5根K线前布林带宽度：{m15_bandwidth_5bars_ago}%
- 布林带外K线数：{m15_bars_outside_band}
- 15m涨跌幅：{m15_change}%
- K线实体：{m15_candle_body}
- 成交量（上一根已完成K线）：{m15_volume}（20周期均量：{m15_volume_sma20}）
- 20周期最高/最低：{m15_high_20} / {m15_low_20}
- 50周期最高/最低：{m15_high_50} / {m15_low_50}

## 统计
- 总交易次数：{total_trades}
- 盈利/亏损：{win_trades}/{loss_trades}
- 累计盈亏：{total_pnl} USDT
- 连续亏损次数：{consecutive_losses}

## 触发原因
{trigger_reason}
