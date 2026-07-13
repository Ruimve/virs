import { memo } from 'react';
import { formatSmart } from '../../components/utils/utils';
import { Badge } from '@/components/Badge';
import { StateFeedback } from '@/components/StateFeedback';

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
}

const GridLevelsTab = ({ gridLevels }: GridLevelsTabProps) => {
  if (gridLevels.length === 0) {
    return <StateFeedback type="empty" text="暂无网格数据" />;
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
              const isClosed =
                level.buy_filled && level.sell_filled && Math.abs(level.hold_quantity) === 0;
              return (
                <tr
                  key={level.level}
                  className={`border-b border-line-subtle ${isHolding ? 'bg-success/5' : isClosed ? 'bg-base-secondary/50' : ''}`}
                >
                  <td className="px-4 py-2 text-on-surface-secondary font-mono">{level.level}</td>
                  <td className="px-3 py-2 text-center">
                    <Badge variant={level.side === 'buy' ? 'success' : 'danger'}>
                      {level.side === 'buy' ? '多' : '空'}
                    </Badge>
                  </td>
                  <td className="px-3 py-2 text-success-text text-right font-mono">
                    {level.buy_price.toFixed(2)}
                  </td>
                  <td className="px-3 py-2 text-danger-text text-right font-mono">
                    {level.sell_price.toFixed(2)}
                  </td>
                  <td className="px-3 py-2 text-on-surface text-right font-mono">
                    {level.last_fill_price > 0 ? level.last_fill_price.toFixed(2) : '-'}
                  </td>
                  <td className="px-3 py-2 text-on-surface-secondary text-right font-mono">
                    {Math.abs(level.hold_quantity) > 0
                      ? `${formatSmart(Math.abs(level.hold_quantity))}`
                      : '-'}
                  </td>
                  <td className="px-3 py-2 text-center">
                    {isClosed ? (
                      <span
                        className="inline-block w-2 h-2 rounded-full bg-on-surface-muted"
                        title="已平仓"
                      />
                    ) : isHolding ? (
                      <span
                        className="inline-block w-2 h-2 rounded-full bg-success"
                        title="持仓中"
                      />
                    ) : (
                      <span
                        className="inline-block w-2 h-2 rounded-full bg-on-surface-faint"
                        title="等待"
                      />
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
};

export default memo(GridLevelsTab);
