import { formatPnlShort } from '../../components/utils/utils'

interface GridBot {
  total_pnl: number
  unrealized_pnl: number
  total_trades: number
  grid_count: number
  grid_filled_count: number
  grid_profit_pct: number
  upper_price: number
  lower_price: number
}

interface GridStatsProps {
  bot: GridBot
}

export default function GridStats({ bot }: GridStatsProps) {
  const b = bot

  return (
    <div className="px-3 py-2 flex items-center gap-3 overflow-x-auto text-xs scrollbar-none">
      <span className="shrink-0">已实现 {formatPnlShort(b.total_pnl)}</span>
      <span className="shrink-0">未实现 {formatPnlShort(b.unrealized_pnl)}</span>

      <span className="w-px h-3 bg-line-subtle shrink-0" />

      <span className="text-on-surface-tertiary shrink-0">{b.total_trades}笔</span>
      <span className="text-on-surface-tertiary shrink-0">
        填充{' '}
        <span className="text-on-surface font-mono">
          {Math.min(b.grid_filled_count, b.grid_count)}/{b.grid_count}
        </span>
      </span>

      <span className="w-px h-3 bg-line-subtle shrink-0" />

      <span className="text-on-surface-tertiary shrink-0">
        利润率 <span className="text-on-surface font-mono">{b.grid_profit_pct}%</span>
      </span>
      {b.upper_price > 0 && (
        <span className="text-on-surface-tertiary shrink-0">
          区间{' '}
          <span className="text-on-surface font-mono">
            {b.lower_price.toFixed(0)}—{b.upper_price.toFixed(0)}
          </span>
        </span>
      )}
    </div>
  )
}
