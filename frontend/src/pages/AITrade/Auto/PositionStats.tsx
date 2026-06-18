import { formatPnl } from '../components/shared';

interface AutoBot {
  current_side: string | null;
  entry_price: number;
  position_size: number;
  unrealized_pnl: number;
  total_pnl: number;
  total_trades: number;
  win_trades: number;
}

interface PositionStatsProps {
  bot: AutoBot;
}

export default function PositionStats({ bot }: PositionStatsProps) {
  const b = bot;
  const hasPosition = b.current_side && b.current_side !== 'none';
  const winRate = b.total_trades > 0 ? ((b.win_trades / b.total_trades) * 100).toFixed(1) + '%' : '-';

  return (
    <div className="px-3 py-2 flex items-center gap-3 overflow-x-auto text-xs scrollbar-none">
      {/* Position */}
      {hasPosition ? (
        <>
          <span className={`font-medium ${b.current_side === 'long' ? 'text-emerald-400' : 'text-red-400'}`}>
            {b.current_side === 'long' ? '做多' : '做空'}
          </span>
          <span className="text-on-surface-tertiary">
            入场 <span className="text-on-surface font-mono">{b.entry_price.toFixed(2)}</span>
          </span>
          <span className="text-on-surface-tertiary">
            量 <span className="text-on-surface font-mono">{b.position_size.toFixed(4)}</span>
          </span>
          <span className="shrink-0">
            {formatPnl(b.unrealized_pnl)}
          </span>
        </>
      ) : (
        <span className="text-on-surface-tertiary">无持仓</span>
      )}

      <span className="w-px h-3 bg-line-subtle shrink-0" />

      {/* Stats */}
      <span className="shrink-0">
        累计 {formatPnl(b.total_pnl)}
      </span>
      <span className="text-on-surface-tertiary shrink-0">
        {b.total_trades}笔 · 胜率 <span className="text-on-surface font-mono">{winRate}</span>
      </span>
    </div>
  );
}
