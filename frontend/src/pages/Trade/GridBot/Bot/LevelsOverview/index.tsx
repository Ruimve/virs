import { memo } from 'react';
import type { GridLevelInfo } from '@/service';
import PanelSection from '@/components/PanelSection';
import Badge from '@/components/Badge';
import { formatSmart } from '../../../components/utils/utils';

interface Props {
  gridLevels: GridLevelInfo[];
}

const LevelsOverview = ({ gridLevels }: Props) => {
  const holding = gridLevels.filter((l) => Math.abs(l.hold_quantity) > 0);
  const closed = gridLevels.filter(
    (l) => l.buy_filled && l.sell_filled && Math.abs(l.hold_quantity) === 0,
  );
  const waiting = gridLevels.length - holding.length - closed.length;

  return (
    <PanelSection
      title="网格层级"
      count={gridLevels.length}
      empty={gridLevels.length === 0}
      emptyText="暂无网格数据"
    >
      <>
        {/* 统计摘要 */}
        <div className="grid grid-cols-3 gap-2 px-3 py-2 border-b border-line-subtle">
          <div className="text-center">
            <div className="text-sm font-mono tabular-nums font-medium text-success-text">
              {holding.length}
            </div>
            <div className="text-[11px] text-on-surface-tertiary">持仓</div>
          </div>
          <div className="text-center">
            <div className="text-sm font-mono tabular-nums font-medium text-on-surface-muted">
              {closed.length}
            </div>
            <div className="text-[11px] text-on-surface-tertiary">已平</div>
          </div>
          <div className="text-center">
            <div className="text-sm font-mono tabular-nums font-medium text-on-surface-faint">
              {waiting}
            </div>
            <div className="text-[11px] text-on-surface-tertiary">等待</div>
          </div>
        </div>
        {/* 层级列表 */}
        <div className="divide-y divide-line-subtle">
          {gridLevels.slice(0, 20).map((level) => {
            const isHolding = Math.abs(level.hold_quantity) > 0;
            const isClosed =
              level.buy_filled && level.sell_filled && Math.abs(level.hold_quantity) === 0;
            return (
              <div key={level.level} className="px-3 py-1.5 flex items-center gap-2 text-sm">
                <span className="text-on-surface-tertiary font-mono tabular-nums w-6 shrink-0">
                  L{level.level}
                </span>
                <Badge variant={level.side === 'buy' ? 'success' : 'danger'}>
                  {level.side === 'buy' ? '多' : '空'}
                </Badge>
                <span className="text-on-surface font-mono tabular-nums flex-1 truncate">
                  {level.buy_price.toFixed(2)} → {level.sell_price.toFixed(2)}
                </span>
                {isHolding && (
                  <span className="text-success-text font-mono tabular-nums text-[11px] shrink-0">
                    {formatSmart(Math.abs(level.hold_quantity))}
                  </span>
                )}
                {isClosed && (
                  <span className="w-1.5 h-1.5 rounded-full bg-on-surface-muted shrink-0" />
                )}
                {!isHolding && !isClosed && (
                  <span className="w-1.5 h-1.5 rounded-full bg-on-surface-faint shrink-0" />
                )}
              </div>
            );
          })}
        </div>
      </>
    </PanelSection>
  );
};

export default memo(LevelsOverview);
