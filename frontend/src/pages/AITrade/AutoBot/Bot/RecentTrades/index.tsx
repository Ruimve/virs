import { memo } from 'react'
import type { AutoTrade } from '@/service'
import { formatPnl } from '../../../components/utils/utils'

interface Props {
  trades: AutoTrade[]
}

const RecentTrades = ({ trades }: Props) => {
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
            {recent.map((t) => {
              const isClosed = t.status === 'closed'
              const totalFee = t.open_fee + t.close_fee
              return (
                <div key={t.id} className="px-3 py-2">
                  <div className="flex items-center justify-between mb-0.5">
                    <span
                      className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${
                        t.status === 'open'
                          ? 'text-amber-400'
                          : t.close_reason === 'stop_loss'
                            ? 'text-red-400'
                            : t.close_reason === 'take_profit'
                              ? 'text-emerald-400'
                              : 'text-on-surface-tertiary'
                      } bg-surface-2`}
                    >
                      {t.status === 'open'
                        ? '持仓中'
                        : t.close_reason === 'stop_loss'
                          ? '止损'
                          : t.close_reason === 'take_profit'
                            ? '止盈'
                            : t.close_reason === 'position_timeout'
                              ? '超时'
                              : '已平仓'}
                    </span>
                    <span className="text-[10px] text-on-surface-tertiary">
                      {new Date(
                        isClosed && t.closed_at ? t.closed_at : t.opened_at,
                      ).toLocaleTimeString('zh-CN', {
                        hour: '2-digit',
                        minute: '2-digit',
                      })}
                    </span>
                  </div>
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="text-on-surface font-mono">
                      {t.open_side === 'buy' ? '开多' : '开空'} {t.open_quantity.toFixed(4)} @{' '}
                      {t.open_price.toFixed(2)}
                    </span>
                    {isClosed && <span className="font-mono">{formatPnl(t.pnl)}</span>}
                  </div>
                  {(totalFee > 0 || (isClosed && t.close_price)) && (
                    <div className="text-[10px] text-on-surface-tertiary mt-0.5 font-mono">
                      {isClosed && t.close_price && <span>平仓 @ {t.close_price.toFixed(2)}</span>}
                      {totalFee > 0 && (
                        <span className="text-amber-400 ml-2">费 {totalFee.toFixed(4)}</span>
                      )}
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}

export default memo(RecentTrades)
