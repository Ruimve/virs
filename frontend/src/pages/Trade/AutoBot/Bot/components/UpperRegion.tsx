import { memo } from 'react';
import type { UpperRegionProps } from './types';
import { MarketLine } from './MarketLine';
import { AccountBar } from './AccountBar';
import { StrategyBlock } from './StrategyBlock';

/**
 * 上区容器（surface-1 白色背景）。
 * 组合：市场行 + 账户条 + AI 策略块。
 */
export const UpperRegion = memo(
  ({
    bot,
    latestPrice,
    marketSummary,
    decision,
    stats,
    accountMetrics,
    totalPnl,
    totalPnlPct,
  }: UpperRegionProps) => {
    return (
      <div className="bg-surface-1 border-b border-line-default px-3 py-2.5 flex flex-col gap-2">
        <MarketLine
          symbol={bot.symbol}
          exchange={bot.exchange}
          latestPrice={latestPrice}
          changePct={marketSummary.changePct}
          leverage={bot.leverage}
        />
        <AccountBar metrics={accountMetrics} />
        <StrategyBlock
          bot={bot}
          decision={decision}
          stats={stats}
          totalPnl={totalPnl}
          totalPnlPct={totalPnlPct}
          decideIntervalSecs={bot.decide_interval_secs}
        />
      </div>
    );
  },
);
