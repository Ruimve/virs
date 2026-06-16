import { formatPnlShort } from '../components/shared';

interface GridBot {
  total_pnl: number;
  unrealized_pnl: number;
  total_trades: number;
  grid_count: number;
  grid_filled_count: number;
  grid_profit_pct: number;
  upper_price: number;
  lower_price: number;
}

interface GridStatsProps {
  bot: GridBot;
}

export default function GridStats({ bot }: GridStatsProps) {
  const b = bot;

  return (
    <div className="px-4 pt-4 pb-2 grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-3">
      <div className="rounded-xl border border-line-default bg-surface-1 p-3 shadow-sm">
        <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">已实现盈亏</div>
        <div className="text-sm font-mono">{formatPnlShort(b.total_pnl)}</div>
      </div>
      <div className="rounded-xl border border-line-default bg-surface-1 p-3 shadow-sm">
        <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">未实现盈亏</div>
        <div className="text-sm font-mono">{formatPnlShort(b.unrealized_pnl)}</div>
      </div>
      <div className="rounded-xl border border-line-default bg-surface-1 p-3 shadow-sm">
        <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">成交次数</div>
        <div className="text-sm font-mono text-on-surface">{b.total_trades}</div>
      </div>
      <div className="rounded-xl border border-line-default bg-surface-1 p-3 shadow-sm">
        <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">已填充</div>
        <div className="text-sm font-mono text-on-surface">
          {Math.min(b.grid_filled_count, b.grid_count)}<span className="text-on-surface-tertiary text-xs">/{b.grid_count}</span>
        </div>
      </div>
      <div className="rounded-xl border border-line-default bg-surface-1 p-3 shadow-sm">
        <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">利润率</div>
        <div className="text-sm font-mono text-on-surface">{b.grid_profit_pct}<span className="text-on-surface-tertiary text-xs">%</span></div>
      </div>
      <div className="rounded-xl border border-line-default bg-surface-1 p-3 shadow-sm">
        <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">价格区间</div>
        <div className="text-xs font-mono text-on-surface">
          {b.upper_price > 0 ? (
            <>{b.lower_price.toFixed(0)}<span className="text-on-surface-tertiary"> — </span>{b.upper_price.toFixed(0)}</>
          ) : '-'}
        </div>
      </div>
    </div>
  );
}
