import { memo, useState, type RefObject } from 'react'
import KlineChart, { type KlineChartHandle } from '../../../../components/Chart/KlineChart'
import type { KlineCandle } from '../shared'
import type { OrderBookData } from '../../../../service/types'

interface ChartMarker {
  time: number
  position: 'aboveBar' | 'belowBar' | 'inBar'
  color: string
  shape: 'circle' | 'square' | 'arrowUp' | 'arrowDown'
  text?: string
}

interface CollapsibleMarketPanelProps {
  klineData: KlineCandle[]
  klineTimeframe: string
  onTimeframeChange: (tf: string) => void
  chartRef: RefObject<KlineChartHandle | null>
  markers?: ChartMarker[]
  orderBook: OrderBookData
  latestPrice: number
}

const CollapsibleMarketPanel = ({
  klineData,
  klineTimeframe,
  onTimeframeChange,
  chartRef,
  markers,
  orderBook,
  latestPrice,
}: CollapsibleMarketPanelProps) => {
  const [expanded, setExpanded] = useState(false)

  const bestBid = orderBook.bids[0]?.price
  const bestAsk = orderBook.asks[0]?.price
  const spread = bestBid && bestAsk ? (bestAsk - bestBid).toFixed(2) : '-'

  return (
    <div className="border-t border-line-subtle shrink-0">
      {/* 折叠头部 */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between px-4 py-2 hover:bg-surface-2/50 transition-colors"
      >
        <div className="flex items-center gap-3 text-xs">
          <span className="text-on-surface-tertiary">行情</span>
          {latestPrice > 0 && (
            <span className="text-on-surface font-mono font-medium">{latestPrice.toFixed(2)}</span>
          )}
          {bestBid && bestAsk && (
            <>
              <span className="text-emerald-400 font-mono text-[11px]">{bestBid.toFixed(2)}</span>
              <span className="text-on-surface-muted text-[10px]">spread {spread}</span>
              <span className="text-red-400 font-mono text-[11px]">{bestAsk.toFixed(2)}</span>
            </>
          )}
        </div>
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-on-surface-muted">{expanded ? '收起' : '展开'}</span>
          <svg
            className={`w-3.5 h-3.5 text-on-surface-muted transition-transform ${expanded ? 'rotate-180' : ''}`}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
          </svg>
        </div>
      </button>

      {/* 展开内容：K线图 + 订单簿 */}
      {expanded && (
        <div className="flex flex-col lg:flex-row h-[400px] border-t border-line-subtle">
          {/* K线图 */}
          <div className="flex-1 flex flex-col min-h-0">
            <div className="flex items-center gap-1 px-4 pt-2 pb-1 shrink-0">
              {['1m', '5m', '15m', '1h', '4h', '1d'].map((tf) => (
                <button
                  key={tf}
                  onClick={() => onTimeframeChange(tf)}
                  className={`px-2 py-0.5 rounded text-[10px] font-medium transition-colors ${
                    klineTimeframe === tf
                      ? 'bg-accent-light text-accent'
                      : 'text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2'
                  }`}
                >
                  {tf}
                </button>
              ))}
            </div>
            <div className="flex-1 min-h-0 px-2 pb-2">
              {klineData.length > 0 ? (
                <KlineChart ref={chartRef} data={klineData} markers={markers} />
              ) : (
                <div className="flex items-center justify-center h-full min-h-[200px] text-on-surface-tertiary text-xs">
                  <svg className="animate-spin h-4 w-4 mr-2" viewBox="0 0 24 24" fill="none">
                    <circle
                      className="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      strokeWidth="4"
                    />
                    <path
                      className="opacity-75"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                    />
                  </svg>
                  加载K线...
                </div>
              )}
            </div>
          </div>

          {/* 订单簿 */}
          <div className="lg:w-56 xl:w-64 border-t lg:border-t-0 lg:border-l border-line-subtle shrink-0 max-h-[200px] lg:max-h-none overflow-y-auto">
            <div className="px-3 py-2 border-b border-line-subtle sticky top-0 bg-surface-1">
              <span className="text-[10px] text-on-surface-tertiary uppercase tracking-wider">
                订单簿
              </span>
            </div>
            <div className="grid grid-cols-2 gap-2 text-[10px] font-mono px-3 py-2">
              <div>
                <div className="text-on-surface-tertiary mb-1 text-[9px] uppercase">Bids</div>
                {orderBook.bids.slice(0, 10).map((b, i) => (
                  <div key={i} className="flex justify-between text-emerald-400">
                    <span>{b.price.toFixed(2)}</span>
                    <span>{b.amount.toFixed(4)}</span>
                  </div>
                ))}
              </div>
              <div>
                <div className="text-on-surface-tertiary mb-1 text-[9px] uppercase">Asks</div>
                {orderBook.asks.slice(0, 10).map((a, i) => (
                  <div key={i} className="flex justify-between text-red-400">
                    <span>{a.price.toFixed(2)}</span>
                    <span>{a.amount.toFixed(4)}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export default memo(CollapsibleMarketPanel)
