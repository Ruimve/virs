import { formatSmart } from '../components/shared';

interface GridLevelInfo {
  level: number;
  price: number;
  side: string;
  buy_price: number;
  sell_price: number;
  quantity: number;
  buy_filled: boolean;
  sell_filled: boolean;
  hold_quantity: number;
  avg_buy_price: number;
  last_fill_price: number;
}

interface GridLevelsTabProps {
  gridLevels: GridLevelInfo[];
  loading: boolean;
}

export default function GridLevelsTab({ gridLevels, loading }: GridLevelsTabProps) {
  if (loading) {
    return (
      <div className="flex justify-center py-12">
        <svg className="animate-spin h-5 w-5 text-on-surface-tertiary" viewBox="0 0 24 24" fill="none">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
        </svg>
      </div>
    );
  }

  if (gridLevels.length === 0) {
    return <div className="text-center py-12 text-on-surface-tertiary text-sm">暂无网格数据</div>;
  }

  return (
    <div className="max-w-7xl mx-auto px-4 md:px-8 py-6">
      <div className="rounded-xl border border-line-default bg-surface-1 overflow-hidden shadow-sm">
        <table className="w-full text-xs">
          <thead>
            <tr className="text-on-surface-tertiary border-b border-line-subtle bg-base-secondary">
              <th className="text-left px-4 py-2.5 font-medium">层级</th>
              <th className="text-center px-3 py-2.5 font-medium">方向</th>
              <th className="text-right px-3 py-2.5 font-medium">开仓价</th>
              <th className="text-right px-3 py-2.5 font-medium">平仓价</th>
              <th className="text-right px-3 py-2.5 font-medium">成交价</th>
              <th className="text-right px-3 py-2.5 font-medium">持仓</th>
              <th className="text-center px-3 py-2.5 font-medium">状态</th>
            </tr>
          </thead>
          <tbody>
            {gridLevels.map((level) => {
              const isHolding = Math.abs(level.hold_quantity) > 0;
              const isClosed = level.buy_filled && level.sell_filled && Math.abs(level.hold_quantity) === 0;
              return (
                <tr key={level.level} className={`border-b border-line-subtle ${isHolding ? 'bg-emerald-500/5' : isClosed ? 'bg-base-secondary/50' : ''}`}>
                  <td className="px-4 py-2 text-on-surface-secondary font-mono">{level.level}</td>
                  <td className="px-3 py-2 text-center">
                    <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${level.side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}>
                      {level.side === 'buy' ? '多' : '空'}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-emerald-400 text-right font-mono">{level.buy_price.toFixed(2)}</td>
                  <td className="px-3 py-2 text-red-400 text-right font-mono">{level.sell_price.toFixed(2)}</td>
                  <td className="px-3 py-2 text-on-surface text-right font-mono">{level.last_fill_price > 0 ? level.last_fill_price.toFixed(2) : '-'}</td>
                  <td className="px-3 py-2 text-on-surface-secondary text-right font-mono">
                    {Math.abs(level.hold_quantity) > 0 ? `${formatSmart(Math.abs(level.hold_quantity))}` : '-'}
                  </td>
                  <td className="px-3 py-2 text-center">
                    {isClosed ? (
                      <span className="inline-block w-2 h-2 rounded-full bg-on-surface-muted" title="已平仓" />
                    ) : isHolding ? (
                      <span className="inline-block w-2 h-2 rounded-full bg-emerald-500" title="持仓中" />
                    ) : (
                      <span className="inline-block w-2 h-2 rounded-full bg-on-surface-faint" title="等待" />
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
