import { memo, useEffect, useState } from 'react'
import { getGridTrades, type GridTrade } from '@/service'
import { formatPnlShort } from '../../../components/utils/utils'

interface Props {
  botId: string
}

/** 网格机器人最近成交卡片（右侧栏） */
const RecentTrades = ({ botId }: Props) => {
  const [trades, setTrades] = useState<GridTrade[]>([])

  useEffect(() => {
    if (!botId) return
    let cancelled = false
    getGridTrades(botId, 1, 5)
      .then((res) => {
        if (cancelled) return
        if (res.success && res.data) setTrades(res.data.trades || [])
      })
      .catch((e) => console.error('Failed to load grid trades:', e))
    return () => {
      cancelled = true
    }
  }, [botId])

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
                    className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${t.open_side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}
                  >
                    {t.open_side === 'buy' ? '买入' : '卖出'} L{t.grid_level}
                  </span>
                  <span className="text-[10px] text-on-surface-tertiary">
                    {new Date(t.opened_at).toLocaleTimeString('zh-CN', {
                      hour: '2-digit',
                      minute: '2-digit',
                    })}
                  </span>
                </div>
                <div className="flex items-center justify-between text-[11px]">
                  <span className="text-on-surface font-mono">
                    {t.open_quantity.toFixed(4)} @ {t.open_price.toFixed(2)}
                  </span>
                  {t.status === 'closed' && (
                    <span className="font-mono">{formatPnlShort(t.pnl)}</span>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

export default memo(RecentTrades)
