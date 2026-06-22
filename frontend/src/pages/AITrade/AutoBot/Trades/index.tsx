import type { AutoTrade } from '@/service'
import { memo, useMemo } from 'react'
import { useBot } from '../../context/BotContext'
import { formatPnl, tradeTypeColor, tradeTypeLabel } from '../../components/utils/utils'

const Trades = () => {
  const { trades } = useBot()
  const autoTrades = useMemo(() => trades as AutoTrade[], [trades])
  return (
    <div className="max-w-5xl mx-auto px-4 md:px-8 py-6">
      {autoTrades.length > 0 ? (
        <div className="bg-surface-1 rounded-xl border border-line-default shadow-sm overflow-hidden">
          <div className="divide-y divide-line-subtle">
            {autoTrades.slice(0, 50).map((t) => (
              <div
                key={t.id}
                className="flex items-center justify-between px-5 py-3 hover:bg-surface-2/50"
              >
                <div className="flex items-center gap-3">
                  <span
                    className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${tradeTypeColor(t.trade_type)} bg-surface-2`}
                  >
                    {tradeTypeLabel(t.trade_type)}
                  </span>
                  <div>
                    <div className="text-xs text-on-surface font-mono">
                      {t.side === 'buy' ? '买入' : '卖出'} {t.quantity.toFixed(6)} @{' '}
                      {t.price.toFixed(2)}
                    </div>
                    <div className="text-[10px] text-on-surface-tertiary mt-0.5">
                      {new Date(t.created_at).toLocaleString('zh-CN')}
                      {t.fee > 0 && (
                        <span className="text-amber-400 ml-2 font-mono">
                          手续费 {t.fee.toFixed(4)}
                        </span>
                      )}
                    </div>
                  </div>
                </div>
                {t.pnl !== 0 && (
                  <div className="text-right">
                    {formatPnl(t.pnl)}
                    {t.pnl_pct !== 0 && (
                      <div className="text-[10px] text-on-surface-tertiary font-mono">
                        {t.pnl_pct.toFixed(2)} %
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      ) : (
        <div className="text-center py-12 text-on-surface-tertiary text-xs"> 暂无交易记录 </div>
      )}
    </div>
  )
}

export default memo(Trades)
