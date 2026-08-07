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

## 4小时大趋势
- EMA20：{h4_ema20}
- EMA50：{h4_ema50}
- RSI(14)：{h4_rsi}
- MACD柱状图：{h4_macd_histogram}
- ADX(14)：{h4_adx}

## 1小时主周期
- 当前价格：{h1_current_price}
- EMA20：{h1_ema20}
- EMA50：{h1_ema50}
- EMA交叉状态：{h1_ema_cross}
- EMA交叉距现在：{h1_ema_cross_bars_ago}根K线（-1=无交叉，0-5=新鲜信号）
- EMA间距：{h1_ema_gap_pct}%（{h1_ema_gap_trend}）
- RSI(14)：{h1_rsi}
- MACD：{h1_macd}
- MACD信号线：{h1_macd_signal}
- MACD柱状图：{h1_macd_histogram}
- ADX(14)：{h1_adx}
- ATR(14)：{h1_atr}
- 布林带上/中/下轨：{h1_bb_upper} / {h1_bb_middle} / {h1_bb_lower}
- 布林带宽度：{h1_bb_width_pct}%
- 1h涨跌幅：{h1_change}%
- 成交量（上一根已完成K线）：{h1_volume}（20周期均量：{h1_volume_sma20}）
- 50周期最高/最低：{h1_high_50} / {h1_low_50}

## 15分钟入场周期
- 当前价格：{m15_current_price}
- EMA20：{m15_ema20}
- EMA50：{m15_ema50}
- EMA交叉状态：{m15_ema_cross}
- EMA交叉距现在：{m15_ema_cross_bars_ago}根K线
- RSI(14)：{m15_rsi}
- MACD：{m15_macd}
- MACD信号线：{m15_macd_signal}
- MACD柱状图：{m15_macd_histogram}
- ATR(14)：{m15_atr}
- ADX(14)：{m15_adx}
- 成交量（上一根已完成K线）：{m15_volume}（20周期均量：{m15_volume_sma20}）
- 50周期最高/最低：{m15_high_50} / {m15_low_50}

## 统计
- 总交易次数：{total_trades}
- 盈利/亏损：{win_trades}/{loss_trades}
- 累计盈亏：{total_pnl} USDT
- 连续亏损次数：{consecutive_losses}

## 触发原因
{trigger_reason}