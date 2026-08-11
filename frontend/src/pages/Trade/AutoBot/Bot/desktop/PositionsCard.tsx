import { memo, useMemo } from 'react';
import type { PositionsCardProps } from './types';
import { formatCompact, computeLiqDistance } from '../components/utils';

function PositionRow({
  side,
  bot,
  position,
  metrics,
  latestPrice,
}: {
  side: 'long' | 'short';
  bot: PositionsCardProps['bot'];
  position: NonNullable<PositionsCardProps['longPosition']>;
  metrics: NonNullable<PositionsCardProps['longMetrics']>;
  latestPrice: number;
}) {
  const isLong = side === 'long';
  const dirLabel = isLong ? 'LONG' : 'SHORT';
  const dirClass = isLong ? 'text-success-text' : 'text-danger-text';
  const dirBg = isLong ? 'bg-success-bg' : 'bg-danger-bg';

  const pnlClass = useMemo(
    () =>
      metrics.unrealizedPnl > 0
        ? 'text-success-text'
        : metrics.unrealizedPnl < 0
          ? 'text-danger-text'
          : 'text-on-surface',
    [metrics.unrealizedPnl],
  );

  const liqDist = computeLiqDistance(latestPrice, metrics.estimatedLiqPrice, side);
  const hasSL = position.stopLoss != null && position.stopLoss > 0;
  const hasTP = position.takeProfit != null && position.takeProfit > 0;

  return (
    <div className="flex flex-col gap-1.5 p-2 rounded-md bg-surface-2/50">
      {}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <span
            className={`inline-block px-1.5 py-0.5 rounded text-2xs font-bold ${dirBg} ${dirClass}`}
          >
            {dirLabel}
          </span>
          <span className="text-2xs font-semibold text-on-surface">{bot.symbol}</span>
        </div>
        <span className={`text-xs font-mono font-bold tabular-nums ${pnlClass}`}>
          {metrics.unrealizedPnl > 0 ? '+' : ''}
          {formatCompact(metrics.unrealizedPnl)}
          <span className="text-2xs ml-1">
            ({metrics.unrealizedPnl > 0 ? '+' : ''}
            {metrics.unrealizedPct.toFixed(2)}%)
          </span>
        </span>
      </div>

      {}
      <div className="grid grid-cols-2 gap-x-3 gap-y-1 text-2xs">
        <div className="flex items-center justify-between">
          <span className="text-on-surface-tertiary">数量</span>
          <span className="font-mono tabular-nums text-on-surface">{position.quantity}</span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-on-surface-tertiary">开仓价</span>
          <span className="font-mono tabular-nums text-on-surface">
            {formatCompact(position.entryPrice)}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-on-surface-tertiary">止损</span>
          <span
            className={`font-mono tabular-nums ${hasSL ? 'text-danger-text' : 'text-on-surface-muted'}`}
          >
            {hasSL ? formatCompact(position.stopLoss!) : '—'}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-on-surface-tertiary">止盈</span>
          <span
            className={`font-mono tabular-nums ${hasTP ? 'text-success-text' : 'text-on-surface-muted'}`}
          >
            {hasTP ? formatCompact(position.takeProfit!) : '—'}
          </span>
        </div>
      </div>

      {}
      {metrics.estimatedLiqPrice && metrics.estimatedLiqPrice > 0 && (
        <div className="flex items-center justify-between px-2 py-1 rounded bg-danger-bg/50 text-2xs">
          <span className="font-semibold text-danger-text">强平价</span>
          <span className="font-bold font-mono tabular-nums text-danger-text">
            {formatCompact(metrics.estimatedLiqPrice)}
          </span>
          {liqDist != null && (
            <span className="text-on-surface-tertiary">距强平 {liqDist.toFixed(1)}%</span>
          )}
        </div>
      )}
    </div>
  );
}

function EmptyPosition({ side }: { side: 'long' | 'short' }) {
  const isLong = side === 'long';
  const dirLabel = isLong ? 'LONG' : 'SHORT';
  const dirClass = isLong ? 'text-success-text' : 'text-danger-text';
  const dirBg = isLong ? 'bg-success-bg/50' : 'bg-danger-bg/50';

  return (
    <div className="flex flex-col gap-1.5 p-2 rounded-md bg-surface-2/30 opacity-70">
      <div className="flex items-center gap-1.5">
        <span
          className={`inline-block px-1.5 py-0.5 rounded text-2xs font-bold ${dirBg} ${dirClass}`}
        >
          {dirLabel}
        </span>
        <span className="text-2xs text-on-surface-muted">observing</span>
      </div>
      <span className="text-2xs text-on-surface-muted">
        {isLong ? '暂无多头仓位' : '暂无空头仓位'}
      </span>
    </div>
  );
}

export const PositionsCard = memo(
  ({
    bot,
    longPosition,
    shortPosition,
    longMetrics,
    shortMetrics,
    latestPrice,
  }: PositionsCardProps) => {
    const hasAny = longMetrics.hasPosition || shortMetrics.hasPosition;

    return (
      <div className="rounded-lg border border-line-default bg-surface-1 p-3 flex flex-col gap-2">
        {}
        <div className="flex items-center justify-between">
          <span className="text-xs font-semibold text-on-surface">持仓管理</span>
          <span className="text-2xs text-on-surface-muted font-mono">Hedge Mode</span>
        </div>

        {}
        {hasAny ? (
          <div className="flex flex-col gap-2">
            {longMetrics.hasPosition && longPosition ? (
              <PositionRow
                side="long"
                bot={bot}
                position={longPosition}
                metrics={longMetrics}
                latestPrice={latestPrice}
              />
            ) : (
              <EmptyPosition side="long" />
            )}
            {shortMetrics.hasPosition && shortPosition ? (
              <PositionRow
                side="short"
                bot={bot}
                position={shortPosition}
                metrics={shortMetrics}
                latestPrice={latestPrice}
              />
            ) : (
              <EmptyPosition side="short" />
            )}
          </div>
        ) : (
          <div className="flex items-center justify-center py-4">
            <span className="text-xs text-on-surface-muted">暂无持仓</span>
          </div>
        )}
      </div>
    );
  },
);
