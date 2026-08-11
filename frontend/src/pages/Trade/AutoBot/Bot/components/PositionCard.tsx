import { memo } from 'react';
import type { AnalysisLog } from '@/service/types';
import { RiskHeatBar } from '@/components/RiskHeatBar';
import type { PositionCardProps } from './types';
import { formatCompact, computeLiqDistance } from './utils';

function extractEmptyReason(decision: AnalysisLog | null, side: 'long' | 'short'): string | null {
  if (!decision?.result) return null;
  const dec =
    decision.result.decision ??
    (decision.result.action
      ? {
          action: decision.result.action,
          reason: decision.result.reason ?? '',
          confidence: decision.result.confidence ?? 0,
        }
      : null);
  if (!dec) return null;

  const oppositeAction = side === 'short' ? 'open_long' : 'open_short';
  if (dec.action === oppositeAction || dec.action === 'hold') {
    const conf = dec.confidence != null ? `置信度 ${dec.confidence.toFixed(2)}` : '';
    return dec.reason ? `${dec.reason}${conf ? ` · ${conf}` : ''}` : conf || null;
  }
  return null;
}

function formatHoldDuration(createdAt: string): string {
  const now = Date.now();
  const opened = new Date(createdAt).getTime();
  if (isNaN(opened) || opened > now) return '';
  const ms = now - opened;
  const h = Math.floor(ms / 3600000);
  const m = Math.floor((ms % 3600000) / 60000);
  if (h > 0) return `holding · ${h}h${m}m`;
  return `holding · ${m}m`;
}

export const PositionCard = memo(
  ({ side, position, metrics, latestPrice, decision, realizedPnl }: PositionCardProps) => {
    const isLong = side === 'long';
    const dirLabel = isLong ? '▲ 多头' : '▼ 空头';
    const dirClass = isLong ? 'text-success-text' : 'text-danger-text';

    if (!metrics.hasPosition || !position) {
      const emptyReason = extractEmptyReason(decision, side);
      const realizedText =
        realizedPnl !== 0
          ? `已实现 ${realizedPnl > 0 ? '+' : ''}${formatCompact(realizedPnl)}`
          : null;

      return (
        <div className="flex-1 bg-surface-1 border border-line-default opacity-70 rounded-lg p-2.5 flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <span className={`text-xs font-bold ${dirClass}`}>{dirLabel}</span>
            <span className="text-2xs text-on-surface-muted font-mono">observing</span>
          </div>
          <div className="text-2xs text-on-surface-muted leading-relaxed">
            {emptyReason ?? (isLong ? '暂无多头仓位' : '暂无空头仓位')}
            {realizedText && (
              <span className="text-on-surface-tertiary font-mono tabular-nums">
                {' '}
                · {realizedText}
              </span>
            )}
          </div>
        </div>
      );
    }

    const pnlClass =
      metrics.unrealizedPnl > 0
        ? 'text-success-text'
        : metrics.unrealizedPnl < 0
          ? 'text-danger-text'
          : 'text-on-surface';
    const liqDist = computeLiqDistance(latestPrice, metrics.estimatedLiqPrice, side);
    const liqPrice = metrics.estimatedLiqPrice;
    const holdDuration = formatHoldDuration(position.createdAt);
    const hasSL = position.stopLoss != null && position.stopLoss > 0;
    const hasTP = position.takeProfit != null && position.takeProfit > 0;

    return (
      <div className="flex-1 bg-surface-1 border border-line-default rounded-lg p-2.5 flex flex-col gap-2">
        {}
        <div className="flex items-center justify-between">
          <span className={`text-xs font-bold ${dirClass}`}>{dirLabel}</span>
          <span className="text-2xs text-on-surface-muted font-mono">
            {holdDuration || 'holding'}
          </span>
        </div>

        {}
        <div className="flex items-baseline justify-between">
          <span className="text-2xs text-on-surface-tertiary uppercase tracking-wider">未实现</span>
          <div className="flex items-baseline gap-1.5">
            <span className={`text-base font-bold font-mono tabular-nums ${pnlClass}`}>
              {metrics.unrealizedPnl > 0 ? '+' : ''}
              {formatCompact(metrics.unrealizedPnl)}
            </span>
            <span className={`text-2xs font-semibold font-mono tabular-nums ${pnlClass}`}>
              {metrics.unrealizedPnl > 0 ? '+' : ''}
              {metrics.unrealizedPct.toFixed(2)}%
            </span>
          </div>
        </div>

        {}
        <div className="grid grid-cols-2 gap-px bg-line-subtle rounded overflow-hidden">
          <div className="bg-surface-1 px-2 py-1 flex flex-col gap-0.5">
            <span className="text-2xs uppercase tracking-wide text-on-surface-tertiary">
              持仓量
            </span>
            <span className="text-xs font-semibold font-mono tabular-nums">
              {position.quantity}
            </span>
          </div>
          <div className="bg-surface-1 px-2 py-1 flex flex-col gap-0.5">
            <span className="text-2xs uppercase tracking-wide text-on-surface-tertiary">
              开仓价
            </span>
            <span className="text-xs font-semibold font-mono tabular-nums">
              {formatCompact(position.entryPrice)}
            </span>
          </div>
          <div className="bg-surface-1 px-2 py-1 flex flex-col gap-0.5">
            <span className="text-2xs uppercase tracking-wide text-on-surface-tertiary">止损</span>
            <span
              className={`text-xs font-semibold font-mono tabular-nums ${hasSL ? 'text-danger-text' : 'text-on-surface-muted'}`}
            >
              {hasSL ? formatCompact(position.stopLoss!) : '—'}
            </span>
          </div>
          <div className="bg-surface-1 px-2 py-1 flex flex-col gap-0.5">
            <span className="text-2xs uppercase tracking-wide text-on-surface-tertiary">止盈</span>
            <span
              className={`text-xs font-semibold font-mono tabular-nums ${hasTP ? 'text-success-text' : 'text-on-surface-muted'}`}
            >
              {hasTP ? formatCompact(position.takeProfit!) : '—'}
            </span>
          </div>
        </div>

        {}
        {liqPrice && liqPrice > 0 && (
          <div className="flex items-center justify-between px-2 py-1 rounded bg-danger-bg text-2xs">
            <span className="font-semibold text-danger-text">强平价</span>
            <span className="font-bold font-mono tabular-nums text-danger-text">
              {formatCompact(liqPrice)}
            </span>
            {liqDist != null && (
              <span className="text-on-surface-tertiary">距强平 {liqDist.toFixed(1)}%</span>
            )}
          </div>
        )}

        {}
        <RiskHeatBar
          side={side}
          currentPrice={latestPrice}
          stopLoss={position.stopLoss ?? undefined}
          takeProfit={position.takeProfit ?? undefined}
          entryPrice={position.entryPrice}
        />
      </div>
    );
  },
);
