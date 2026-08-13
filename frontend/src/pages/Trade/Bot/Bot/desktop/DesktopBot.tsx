import { memo, useMemo, useState } from 'react';
import { KlineChart } from '@/components/Chart/KlineChart';
import { TradeLoading } from '../../../components/Transition';
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

  const [rightTab, setRightTab] = useState<'analysis' | 'trading'>('analysis');

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

      {/* Right side panel: tabbed cards (分析 / 交易) */}
      <div className="flex flex-col h-full overflow-hidden bg-surface-1">
        {/* Tab bar */}
        <div className="flex items-center gap-1 px-2 pt-2 pb-1 shrink-0 border-b border-line-subtle">
          <button
            type="button"
            onClick={() => setRightTab('analysis')}
            className={`px-2 py-0.5 rounded text-2xs font-medium transition-colors cursor-pointer ${
              rightTab === 'analysis'
                ? 'bg-accent-light text-accent'
                : 'text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2'
            }`}
          >
            分析
          </button>
          <button
            type="button"
            onClick={() => setRightTab('trading')}
            className={`px-2 py-0.5 rounded text-2xs font-medium transition-colors cursor-pointer ${
              rightTab === 'trading'
                ? 'bg-accent-light text-accent'
                : 'text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2'
            }`}
          >
            交易
          </button>
        </div>

        {/* Tab content */}
        <div className="flex-1 min-h-0 overflow-y-auto p-2 flex flex-col gap-2">
          {rightTab === 'analysis' && (
            <>
              <AIPanel
                decision={decision}
                logs={logs}
                decideIntervalSecs={bot.decide_interval_secs}
              />
              <RiskPanel
                accountMetrics={accountMetrics}
                bot={bot}
                longMetrics={longMetrics}
                shortMetrics={shortMetrics}
              />
            </>
          )}
          {rightTab === 'trading' && (
            <>
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
            </>
          )}
        </div>
      </div>
    </div>
  );
});
