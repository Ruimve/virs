import { memo, useMemo } from 'react'
import { useBot } from '../../context/BotContext'
import type { GridTrade } from '@/service'
import { formatPnlShort } from '../../components/utils/utils'

const Trades = () => {
  const { trades, loading } = useBot()

  const gridTrades = useMemo(() => trades as GridTrade[], [trades])

  return (
    <div className="max-w-7xl mx-auto px-4 md:px-8 py-6">
      {loading ? (
        <div className="flex justify-center py-12">
          <svg
            className="animate-spin h-5 w-5 text-on-surface-tertiary"
            viewBox="0 0 24 24"
            fill="none"
          >
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
        </div>
      ) : gridTrades.length > 0 ? (
        <div className="rounded-xl border border-line-default bg-surface-1 overflow-hidden shadow-sm">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-on-surface-tertiary border-b border-line-subtle bg-base-secondary">
                <th className="text-right px-4 py-2.5 font-medium">层级</th>
                <th className="text-left px-3 py-2.5 font-medium">开仓</th>
                <th className="text-right px-3 py-2.5 font-medium">开仓价</th>
                <th className="text-right px-3 py-2.5 font-medium">开仓量</th>
                <th className="text-left px-3 py-2.5 font-medium">平仓</th>
                <th className="text-right px-3 py-2.5 font-medium">平仓价</th>
                <th className="text-right px-3 py-2.5 font-medium">平仓量</th>
                <th className="text-right px-3 py-2.5 font-medium">盈亏</th>
                <th className="text-left px-3 py-2.5 font-medium">状态</th>
              </tr>
            </thead>
            <tbody>
              {gridTrades.map((t) => (
                <tr key={t.id} className="border-b border-line-subtle">
                  <td className="px-4 py-2 text-on-surface-secondary font-mono text-right">
                    {t.grid_level}
                  </td>
                  <td className="px-3 py-2">
                    <span
                      className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${t.open_side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}
                    >
                      {t.open_side === 'buy' ? '买入' : '卖出'}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-on-surface font-mono text-right">
                    {t.open_price.toFixed(2)}
                  </td>
                  <td className="px-3 py-2 text-on-surface-secondary font-mono text-right">
                    {t.open_quantity.toFixed(4)}
                  </td>
                  <td className="px-3 py-2">
                    {t.close_side ? (
                      <span
                        className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${t.close_side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}
                      >
                        {t.close_side === 'buy' ? '买入' : '卖出'}
                      </span>
                    ) : (
                      <span className="text-on-surface-faint">-</span>
                    )}
                  </td>
                  <td className="px-3 py-2 text-on-surface font-mono text-right">
                    {t.close_price?.toFixed(2) || '-'}
                  </td>
                  <td className="px-3 py-2 text-on-surface-secondary font-mono text-right">
                    {t.close_quantity?.toFixed(4) || '-'}
                  </td>
                  <td className="px-3 py-2 text-right">{formatPnlShort(t.pnl)}</td>
                  <td className="px-3 py-2">
                    <span
                      className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${
                        t.status === 'closed'
                          ? 'bg-emerald-500/10 text-emerald-400'
                          : t.status === 'open'
                            ? 'bg-blue-500/10 text-blue-400'
                            : 'bg-surface-2 text-on-surface-tertiary'
                      }`}
                    >
                      {t.status === 'closed' ? '已平' : t.status === 'open' ? '持仓' : t.status}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="text-center py-12 text-on-surface-tertiary text-sm">暂无交易记录</div>
      )}
    </div>
  )
}

export default memo(Trades)
