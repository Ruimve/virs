import { memo, useState } from 'react'
import type { OrderBookData } from '../../../../service/types'

interface CollapsibleOrderBookProps {
  orderBook: OrderBookData
  /** 默认是否展开，默认 false（收起） */
  defaultExpanded?: boolean
}

const CollapsibleOrderBook = ({
  orderBook,
  defaultExpanded = false,
}: CollapsibleOrderBookProps) => {
  const [expanded, setExpanded] = useState(defaultExpanded)

  const bestBid = orderBook.bids[0]?.price
  const bestAsk = orderBook.asks[0]?.price
  const spread = bestBid && bestAsk ? (bestAsk - bestBid).toFixed(2) : '-'

  return (
    <div className="border-t border-line-subtle shrink-0">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between px-4 py-2 hover:bg-surface-2/50 transition-colors"
      >
        <div className="flex items-center gap-3 text-xs">
          <span className="text-on-surface-tertiary">订单簿</span>
          {bestBid && bestAsk && (
            <>
              <span className="text-emerald-400 font-mono">{bestBid.toFixed(2)}</span>
              <span className="text-on-surface-muted text-[10px]">spread {spread}</span>
              <span className="text-red-400 font-mono">{bestAsk.toFixed(2)}</span>
            </>
          )}
        </div>
        <svg
          className={`w-3.5 h-3.5 text-on-surface-muted transition-transform ${expanded ? 'rotate-180' : ''}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth="2"
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
        </svg>
      </button>
      {expanded && (
        <div className="px-4 pb-3 max-h-[240px] overflow-y-auto">
          <div className="grid grid-cols-2 gap-2 text-[10px] font-mono">
            {/* Bids */}
            <div>
              <div className="text-on-surface-tertiary mb-1 text-[9px] uppercase">Bids</div>
              {orderBook.bids.slice(0, 10).map((b, i) => (
                <div key={i} className="flex justify-between text-emerald-400">
                  <span>{b.price.toFixed(2)}</span>
                  <span>{b.amount.toFixed(4)}</span>
                </div>
              ))}
            </div>
            {/* Asks */}
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
      )}
    </div>
  )
}

export default memo(CollapsibleOrderBook)
