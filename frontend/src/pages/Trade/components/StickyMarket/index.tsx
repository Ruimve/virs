import { memo, useMemo, useState, type RefObject } from 'react';
import type { AnalysisLog } from '@/service/types';
import type { KlineCandle } from '@/service';
import { type KlineChartHandle, KlineChart } from '@/components/Chart/KlineChart';
import { DecisionTimeline, type DecisionAction } from '@/components/DecisionTimeline';
import { TradeLoading } from '@/components/Transition/Icon';
import { FlashPrice } from '../FlashPrice';

interface ChartMarker {
  time: number;
  position: 'aboveBar' | 'belowBar' | 'inBar';
  color: string;
  shape: 'circle' | 'square' | 'arrowUp' | 'arrowDown';
  text?: string;
}

interface CollapsibleMarketPanelProps {
  klineData: KlineCandle[];
  klineTimeframe: string;
  onTimeframeChange: (tf: string) => void;
  chartRef: RefObject<KlineChartHandle | null>;
  markers?: ChartMarker[];
  latestPrice: number;
  logs?: AnalysisLog[];
}

function formatVolume(v: number): string {
  if (v >= 1_000_000_000) return `${(v / 1_000_000_000).toFixed(2)}B`;
  if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(2)}M`;
  if (v >= 1_000) return `${(v / 1_000).toFixed(2)}K`;
  return v.toFixed(2);
}

function formatTime(isoTime: string): string {
  const d = new Date(isoTime);
  if (isNaN(d.getTime())) return '';
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  return `${hh}:${mm}`;
}

function useMarketSummary(klineData: KlineCandle[], timeframe: string) {
  return useMemo(() => {
    if (klineData.length === 0) {
      return { changePct: 0, high: 0, low: 0, volume: 0 };
    }

    const tfHours: Record<string, number> = {
      '1m': 1 / 60,
      '5m': 5 / 60,
      '15m': 15 / 60,
      '1h': 1,
      '4h': 4,
      '1d': 24,
    };
    const hoursPerCandle = tfHours[timeframe] ?? 1;
    const candlesIn24h = Math.min(Math.ceil(24 / hoursPerCandle), klineData.length);

    const recent = klineData.slice(-candlesIn24h);
    const firstClose = recent[0].close;
    const lastClose = recent[recent.length - 1].close;
    const changePct = firstClose > 0 ? ((lastClose - firstClose) / firstClose) * 100 : 0;
    const high = Math.max(...recent.map((k) => k.high));
    const low = Math.min(...recent.map((k) => k.low));
    const volume = recent.reduce((sum, k) => sum + (k.volume || 0), 0);

    return { changePct, high, low, volume };
  }, [klineData, timeframe]);
}

export const StickyMarket = memo(
  ({
    klineData,
    klineTimeframe,
    onTimeframeChange,
    chartRef,
    markers,
    latestPrice,
    logs,
  }: CollapsibleMarketPanelProps) => {
    const [expanded, setExpanded] = useState(false);
    const [view, setView] = useState<'chart' | 'timeline'>('chart');
    const summary = useMarketSummary(klineData, klineTimeframe);

    const timelineItems = useMemo(() => {
      if (!logs) return [];
      return logs.map((log) => {
        const dec =
          log.result?.decision ??
          (log.result?.action
            ? { action: log.result.action, confidence: log.result.confidence ?? 0 }
            : null);
        const action: DecisionAction =
          dec?.action === 'open_long' || dec?.action === 'close_short'
            ? 'buy'
            : dec?.action === 'open_short' || dec?.action === 'close_long'
              ? 'sell'
              : 'hold';
        return {
          time: formatTime(log.created_at),
          action,
          confidence: (dec?.confidence ?? 0) * 100,
          result: undefined,
          resultType: 'pending' as const,
        };
      });
    }, [logs]);

    const changeColor = useMemo(() => {
      return summary.changePct > 0
        ? 'text-success-text'
        : summary.changePct < 0
          ? 'text-danger-text'
          : 'text-on-surface-tertiary';
    }, [summary.changePct]);

    const changeSign = useMemo(() => (summary.changePct > 0 ? '+' : ''), [summary.changePct]);

    const Chart = useMemo(() => {
      return klineData.length > 0 ? (
        <KlineChart ref={chartRef} data={klineData} markers={markers} height={300} />
      ) : (
        <div className="flex flex-col items-center justify-center h-110 gap-3 text-on-surface-tertiary text-xs">
          <TradeLoading size={36} />
          <span className="tracking-wider">加载 K 线</span>
        </div>
      );
    }, [klineData, markers, chartRef]);

    return (
      <div className="bg-base border-t border-line-default shadow-sticky shrink-0">
        <div
          onClick={() => setExpanded(!expanded)}
          className="w-full flex items-center justify-between gap-2 px-4 py-2 hover:bg-surface-2/50 transition-colors cursor-pointer"
        >
          <div className="flex items-center gap-2 sm:gap-3 text-xs min-w-0 overflow-hidden">
            <span className="text-on-surface-tertiary shrink-0">行情</span>
            {latestPrice > 0 && <FlashPrice price={latestPrice} className="text-on-surface" />}
            {summary.changePct !== 0 && (
              <span className={`font-mono text-caption shrink-0 ${changeColor}`}>
                {changeSign}
                {summary.changePct.toFixed(2)}%
              </span>
            )}
            {summary.high > 0 && (
              <>
                <span className="hidden md:block w-px h-3 bg-line-subtle shrink-0" />
                <span className="hidden md:flex items-center gap-1 text-on-surface-tertiary text-caption shrink-0">
                  24H 高{' '}
                  <span className="text-on-surface font-mono">{summary.high.toFixed(2)}</span>
                </span>
                <span className="hidden md:flex items-center gap-1 text-on-surface-tertiary text-caption shrink-0">
                  低 <span className="text-on-surface font-mono">{summary.low.toFixed(2)}</span>
                </span>
                {summary.volume > 0 && (
                  <span className="hidden lg:flex items-center gap-1 text-on-surface-tertiary text-caption shrink-0">
                    量{' '}
                    <span className="text-on-surface font-mono">
                      {formatVolume(summary.volume)}
                    </span>
                  </span>
                )}
              </>
            )}
          </div>
          <div className="flex items-center gap-1.5 shrink-0">
            <span className="hidden sm:inline text-2xs text-on-surface-muted">
              {expanded ? '收起' : '展开'}
            </span>
            <svg
              className={`w-3.5 h-3.5 text-on-surface-muted transition-transform ${expanded ? 'rotate-180' : ''}`}
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
            </svg>
          </div>
        </div>

        {expanded && (
          <div className="flex flex-col border-t border-line-subtle">
            <div className="flex items-center justify-between gap-2 px-4 pt-2 pb-1 shrink-0">
              <div className="flex items-center gap-1">
                <div
                  onClick={(e) => {
                    e.stopPropagation();
                    setView('chart');
                  }}
                  className={`px-2 py-0.5 rounded text-2xs font-medium transition-colors cursor-pointer ${
                    view === 'chart'
                      ? 'bg-accent-light text-accent'
                      : 'text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2'
                  }`}
                >
                  K线
                </div>
                <div
                  onClick={(e) => {
                    e.stopPropagation();
                    setView('timeline');
                  }}
                  className={`px-2 py-0.5 rounded text-2xs font-medium transition-colors cursor-pointer ${
                    view === 'timeline'
                      ? 'bg-accent-light text-accent'
                      : 'text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2'
                  }`}
                >
                  决策
                </div>
              </div>
              {view === 'chart' && (
                <div className="flex items-center gap-1">
                  {['1m', '5m', '15m', '1h', '4h', '1d'].map((tf) => (
                    <div
                      key={tf}
                      onClick={(e) => {
                        e.stopPropagation();
                        onTimeframeChange(tf);
                      }}
                      className={`px-2 py-0.5 rounded text-2xs font-medium transition-colors cursor-pointer ${
                        klineTimeframe === tf
                          ? 'bg-accent-light text-accent'
                          : 'text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2'
                      }`}
                    >
                      {tf}
                    </div>
                  ))}
                </div>
              )}
            </div>
            {view === 'chart' && <div className="px-2 pb-2">{Chart}</div>}
            {view === 'timeline' && (
              <div className="px-4 pb-2 max-h-70 overflow-y-auto">
                {timelineItems.length > 0 ? (
                  <DecisionTimeline items={timelineItems} />
                ) : (
                  <div className="flex items-center justify-center py-8 text-on-surface-muted text-2xs">
                    暂无决策记录
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    );
  },
);
