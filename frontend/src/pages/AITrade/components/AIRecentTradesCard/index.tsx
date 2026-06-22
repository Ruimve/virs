import { memo } from 'react'
import type { AutoTrade } from '@/service'
import { formatPnl, tradeTypeColor, tradeTypeLabel } from '../utils/utils'

interface Props {
  trades: AutoTrade[]
}

const AIRecentTradesCard = ({ trades }: Props) => {
  const recent = trades.slice(0, 5)

  return (
    <div className="flex flex-col min-h-0">
      <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle shrink-0">
        <span className="text-[10px] text-on-surface-tertiary uppercase tracking-wider">
          最近成交
        </span>
        <span className="text-[10px] text-on-surface-muted">{trades.length}</span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {recent.length === 0 ? (
          <div className="text-center py-6 text-[11px] text-on-surface-tertiary">暂无成交</div>
        ) : (
          <div className="divide-y divide-line-subtle">
            {recent.map((t) => (
              <div key={t.id} className="px-3 py-2">
                <div className="flex items-center justify-between mb-0.5">
                  <span
                    className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${tradeTypeColor(t.trade_type)} bg-surface-2`}
                  >
                    {tradeTypeLabel(t.trade_type)}
                  </span>
                  <span className="text-[10px] text-on-surface-tertiary">
                    {new Date(t.created_at).toLocaleTimeString('zh-CN', {
                      hour: '2-digit',
                      minute: '2-digit',
                    })}
                  </span>
                </div>
                <div className="flex items-center justify-between text-[11px]">
                  <span className="text-on-surface font-mono">
                    {t.side === 'buy' ? '买' : '卖'} {t.quantity.toFixed(4)} @ {t.price.toFixed(2)}
                  </span>
                  {t.pnl !== 0 && <span className="font-mono">{formatPnl(t.pnl)}</span>}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

export default memo(AIRecentTradesCard)
