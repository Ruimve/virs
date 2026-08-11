import { memo, useMemo } from 'react';
import { KlineChart } from '@/components/Chart/KlineChart';
import { TradeLoading } from '@/components/Transition/Icon';
import type { DesktopBotProps } from './types';
import { MarketHeader } from './MarketHeader';
import { AIPanel } from './AIPanel';
import { RiskPanel } from './RiskPanel';
import { TradeHistoryTable } from './TradeHistoryTable';
import { AccountCard } from './AccountCard';
import { PositionsCard } from './PositionsCard';

export const DesktopBot = memo((props: DesktopBotProps) => {
  const {
    bot,
    latestPrice,
    marketSummary,
    decision,
    logs,
    trades,
    accountMetrics,
    totalPnl,
    totalPnlPct,
    longPosition,
    shortPosition,
    longMetrics,
    shortMetrics,
    klineData,
    klineTimeframe,
    onTimeframeChange,
    chartRef,
    markers,
  } = props;

  const chartContent = useMemo(() => {
    if (klineData.length === 0) {
      return (
        <div className="flex flex-col items-center justify-center h-full gap-3 text-on-surface-tertiary text-xs">
          <TradeLoading size={36} />
          <span className="tracking-wider">加载 K 线</span>
        </div>
      );
    }
    return <KlineChart ref={chartRef} data={klineData} markers={markers} />;
  }, [klineData, markers, chartRef]);

  return (
    <div className="hidden md:grid h-full w-full overflow-hidden bg-base-secondary grid-cols-[1fr_340px]">
      {/* Left main column: market + chart + trade history */}
      <div className="flex flex-col min-h-0 overflow-hidden border-r border-line-default">
        <MarketHeader
          bot={bot}
          latestPrice={latestPrice}
          marketSummary={marketSummary}
          timeframe={klineTimeframe}
          onTimeframeChange={onTimeframeChange}
        />

        <div className="flex-1 min-h-0 bg-base p-2">{chartContent}</div>

        <div className="shrink-0 p-2 pt-0 max-h-60">
          <TradeHistoryTable trades={trades} />
        </div>
      </div>

      {/* Right side panel: AI + risk + account + positions */}
      <div className="flex flex-col gap-2 overflow-y-auto p-2 bg-surface-1">
        <AIPanel decision={decision} logs={logs} decideIntervalSecs={bot.decide_interval_secs} />
        <RiskPanel
          accountMetrics={accountMetrics}
          bot={bot}
          longMetrics={longMetrics}
          shortMetrics={shortMetrics}
        />
        <AccountCard
          bot={bot}
          accountMetrics={accountMetrics}
          totalPnl={totalPnl}
          totalPnlPct={totalPnlPct}
        />
        <PositionsCard
          bot={bot}
          longPosition={longPosition}
          shortPosition={shortPosition}
          longMetrics={longMetrics}
          shortMetrics={shortMetrics}
          latestPrice={latestPrice}
        />
      </div>
    </div>
  );
});
