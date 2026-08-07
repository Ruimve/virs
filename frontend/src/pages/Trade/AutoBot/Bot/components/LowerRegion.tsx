import { memo } from 'react';
import type { LowerRegionProps } from './types';
import { PositionCard } from './PositionCard';


export const LowerRegion = memo(
  ({
    bot,
    latestPrice,
    longPosition,
    shortPosition,
    longMetrics,
    shortMetrics,
    decision,
  }: LowerRegionProps) => {
    const hasAny = longMetrics.hasPosition || shortMetrics.hasPosition;

    return (
      <div className="bg-base-secondary flex-1 min-h-0 px-3 py-2.5 flex flex-col gap-2 overflow-hidden">
        {hasAny ? (
          <div className="flex flex-col md:flex-row gap-2">
            <PositionCard
              side="long"
              position={longPosition}
              metrics={longMetrics}
              leverage={bot.leverage}
              latestPrice={latestPrice}
              decision={decision}
              realizedPnl={longPosition?.realizedPnl ?? 0}
            />
            <PositionCard
              side="short"
              position={shortPosition}
              metrics={shortMetrics}
              leverage={bot.leverage}
              latestPrice={latestPrice}
              decision={decision}
              realizedPnl={shortPosition?.realizedPnl ?? 0}
            />
          </div>
        ) : (
          <div className="flex-1 flex items-center justify-center min-h-[80px]">
            <span className="text-xs text-on-surface-muted">暂无持仓</span>
          </div>
        )}
      </div>
    );
  },
);
