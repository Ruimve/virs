import { useMemo } from 'react'
import type { KlineCandle, OrderBookData } from '../../../../service/types'

interface MarketIndicatorsProps {
  klineData: KlineCandle[]
  orderBook: OrderBookData
}

function sma(data: KlineCandle[], period: number): number {
  if (data.length < period) return 0
  const slice = data.slice(-period)
  return slice.reduce((s, c) => s + c.close, 0) / period
}

export default function MarketIndicators({ klineData, orderBook }: MarketIndicatorsProps) {
  const indicators = useMemo(() => {
    if (klineData.length < 5) return null

    const last = klineData[klineData.length - 1]
    const prev = klineData[klineData.length - 6]
    const currentPrice = last.close
    const prevPrice = prev.close
    const changePct = ((currentPrice - prevPrice) / prevPrice) * 100

    const ma5 = sma(klineData, 5)
    const ma10 = sma(klineData, 10)
    const trend = ma5 > ma10 ? 'up' : 'down'

    const recent10 = klineData.slice(-10)
    const high = Math.max(...recent10.map((c) => c.high))
    const low = Math.min(...recent10.map((c) => c.low))
    const volatility = ((high - low) / low) * 100

    const bidVol = orderBook.bids.reduce((s, b) => s + b.amount, 0)
    const askVol = orderBook.asks.reduce((s, a) => s + a.amount, 0)
    const totalVol = bidVol + askVol
    const bidPressure = totalVol > 0 ? (bidVol / totalVol) * 100 : 50

    return { currentPrice, changePct, trend, volatility, bidPressure, high, low }
  }, [klineData, orderBook])

  if (!indicators) return null

  const { changePct, trend, volatility, bidPressure, high, low } = indicators

  return (
    <div className="px-4 py-2 border-b border-line-subtle">
      <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">
        市场指标
      </div>
      <div className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-xs">
        {/* 近期涨跌 */}
        <div className="flex items-center justify-between">
          <span className="text-on-surface-tertiary">近期涨跌</span>
          <span className={`font-mono ${changePct >= 0 ? 'text-emerald-400' : 'text-red-400'}`}>
            {changePct >= 0 ? '+' : ''}
            {changePct.toFixed(2)}%
          </span>
        </div>

        {/* 短期趋势 */}
        <div className="flex items-center justify-between">
          <span className="text-on-surface-tertiary">短期趋势</span>
          <span className={`font-medium ${trend === 'up' ? 'text-emerald-400' : 'text-red-400'}`}>
            {trend === 'up' ? '↑ 上升' : '↓ 下降'}
          </span>
        </div>

        {/* 波动幅度 */}
        <div className="flex items-center justify-between">
          <span className="text-on-surface-tertiary">波动幅度</span>
          <span className="text-on-surface font-mono">{volatility.toFixed(2)}%</span>
        </div>

        {/* 买卖压力 */}
        <div className="flex items-center justify-between">
          <span className="text-on-surface-tertiary">买卖压力</span>
          <span
            className={`font-mono ${bidPressure > 55 ? 'text-emerald-400' : bidPressure < 45 ? 'text-red-400' : 'text-on-surface'}`}
          >
            买{bidPressure.toFixed(0)}% / 卖{(100 - bidPressure).toFixed(0)}%
          </span>
        </div>

        {/* 10根高低 */}
        <div className="flex items-center justify-between col-span-2">
          <span className="text-on-surface-tertiary">近期区间</span>
          <span className="text-on-surface font-mono">
            {low.toFixed(2)} — {high.toFixed(2)}
          </span>
        </div>
      </div>
    </div>
  )
}
