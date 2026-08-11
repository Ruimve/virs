import { memo, useMemo } from 'react';

export interface RiskHeatBarProps {
  side: 'long' | 'short';
  currentPrice: number;
  stopLoss?: number;
  takeProfit?: number;
  entryPrice?: number;
  className?: string;
}

const formatPrice = (v: number) => {
  if (v >= 1000) return v.toLocaleString('en-US', { maximumFractionDigits: 0 });
  if (v >= 1) return v.toFixed(2);
  return v.toFixed(4);
};

export const RiskHeatBar = memo(
  ({ side, currentPrice, stopLoss, takeProfit, entryPrice, className = '' }: RiskHeatBarProps) => {
    const isLong = side === 'long';

    const {
      markerPos,
      entryPos,
      showMarker,
      showEntry,
      leftLabel,
      rightLabel,
      leftValue,
      rightValue,
    } = useMemo(() => {
      const sl = stopLoss ?? 0;
      const tp = takeProfit ?? 0;
      const hasRange = stopLoss != null && takeProfit != null && tp !== sl;

      const min = Math.min(sl, tp);
      const max = Math.max(sl, tp);
      const range = max - min || 1;

      const clampPos = (val: number) => Math.min(Math.max((val - min) / range, 0), 1) * 100;

      const leftIsSL = isLong;

      return {
        markerPos: hasRange ? clampPos(currentPrice) : 50,
        entryPos: hasRange && entryPrice != null ? clampPos(entryPrice) : null,
        showMarker: hasRange,
        showEntry: hasRange && entryPrice != null,
        leftLabel: leftIsSL ? 'SL' : 'TP',
        rightLabel: leftIsSL ? 'TP' : 'SL',
        leftValue: leftIsSL ? sl : tp,
        rightValue: leftIsSL ? tp : sl,
      };
    }, [currentPrice, stopLoss, takeProfit, entryPrice, isLong]);

    const segments = isLong
      ? ['bg-danger', 'bg-warning', 'bg-success']
      : ['bg-success', 'bg-warning', 'bg-danger'];

    return (
      <div className={`flex flex-col gap-1.5 ${className}`}>
        {/* Heat bar */}
        <div className="relative">
          <div className="flex h-1.5 rounded-full overflow-hidden">
            {segments.map((color, i) => (
              <div key={i} className={`flex-1 ${color}`} />
            ))}
          </div>
          {/* Entry price marker */}
          {showEntry && entryPos != null && (
            <div
              className="absolute top-0 bottom-0 w-px bg-on-surface-faint opacity-60"
              style={{ left: `${entryPos}%` }}
            />
          )}
          {/* Current price marker */}
          {showMarker && (
            <div
              className="absolute top-1/2 -translate-x-1/2 -translate-y-1/2 w-0.5 h-3.5 bg-on-surface rounded-full"
              style={{ left: `${markerPos}%` }}
            />
          )}
        </div>
        {/* Labels */}
        <div className="flex items-center justify-between text-2xs text-on-surface-muted font-mono tabular-nums">
          <span>{showMarker ? `${leftLabel} ${formatPrice(leftValue)}` : ''}</span>
          <span className="text-on-surface-tertiary font-semibold">
            {formatPrice(currentPrice)}
          </span>
          <span>{showMarker ? `${rightLabel} ${formatPrice(rightValue)}` : ''}</span>
        </div>
      </div>
    );
  },
);
