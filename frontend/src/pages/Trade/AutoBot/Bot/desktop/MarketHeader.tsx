import { memo, useMemo } from 'react';
import type { MarketHeaderProps } from './types';
import { FlashPrice } from '../../../components/FlashPrice';
import { formatCompact } from '../components/utils';

const TIMEFRAMES = ['1m', '5m', '15m', '1h', '4h', '1d'];

function formatVolume(v: number): string {
  if (v >= 1_000_000_000) return `${(v / 1_000_000_000).toFixed(2)}B`;
  if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(2)}M`;
  if (v >= 1_000) return `${(v / 1_000).toFixed(2)}K`;
  return v.toFixed(2);
}

export const MarketHeader = memo(
  ({ bot, latestPrice, marketSummary, timeframe, onTimeframeChange }: MarketHeaderProps) => {
    const chgClass = useMemo(() => {
      return marketSummary.changePct > 0
        ? 'text-success-text'
        : marketSummary.changePct < 0
          ? 'text-danger-text'
          : 'text-on-surface-tertiary';
    }, [marketSummary.changePct]);

    const changeSign = marketSummary.changePct > 0 ? '+' : '';

    return (
      <div className="flex items-center justify-between gap-4 px-4 py-2.5 bg-surface-1 border-b border-line-default shrink-0">
        <div className="flex items-center gap-4 min-w-0">
          <div className="flex flex-col gap-0.5">
            <div className="flex items-baseline gap-1.5">
              <span className="text-base font-bold leading-none tracking-tight text-on-surface">
                {bot.symbol}
              </span>
              <span className="text-2xs text-on-surface-faint">·</span>
              <span className="text-2xs uppercase tracking-wide text-on-surface-tertiary">
                {bot.exchange}
              </span>
              <span className="text-2xs text-on-surface-faint">·</span>
              <span className="text-2xs font-mono font-semibold tabular-nums text-on-surface-secondary">
                {bot.leverage}x
              </span>
            </div>
            <div className="flex items-baseline gap-2">
              <FlashPrice
                price={latestPrice}
                className="text-xl font-bold leading-none tracking-tight text-on-surface"
              />
              <span className={`text-xs font-semibold font-mono tabular-nums ${chgClass}`}>
                {changeSign}
                {marketSummary.changePct.toFixed(2)}%
              </span>
            </div>
          </div>

          {marketSummary.high > 0 && (
            <div className="hidden lg:flex items-center gap-4 pl-4 border-l border-line-subtle">
              <div className="flex flex-col gap-0.5">
                <span className="text-2xs uppercase tracking-wide text-on-surface-tertiary">
                  24H 高
                </span>
                <span className="text-xs font-semibold font-mono tabular-nums text-on-surface">
                  {formatCompact(marketSummary.high)}
                </span>
              </div>
              <div className="flex flex-col gap-0.5">
                <span className="text-2xs uppercase tracking-wide text-on-surface-tertiary">
                  24H 低
                </span>
                <span className="text-xs font-semibold font-mono tabular-nums text-on-surface">
                  {formatCompact(marketSummary.low)}
                </span>
              </div>
              <div className="flex flex-col gap-0.5">
                <span className="text-2xs uppercase tracking-wide text-on-surface-tertiary">
                  24H 量
                </span>
                <span className="text-xs font-semibold font-mono tabular-nums text-on-surface">
                  {formatVolume(marketSummary.volume)}
                </span>
              </div>
            </div>
          )}
        </div>

        <div className="flex items-center gap-0.5 shrink-0">
          {TIMEFRAMES.map((tf) => (
            <button
              key={tf}
              type="button"
              onClick={() => onTimeframeChange(tf)}
              className={`px-2 py-1 rounded text-2xs font-medium transition-colors cursor-pointer ${
                timeframe === tf
                  ? 'bg-accent-light text-accent'
                  : 'text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2'
              }`}
            >
              {tf}
            </button>
          ))}
        </div>
      </div>
    );
  },
);
