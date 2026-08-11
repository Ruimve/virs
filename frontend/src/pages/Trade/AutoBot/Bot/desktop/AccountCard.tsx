import { memo, useMemo } from 'react';
import type { AccountCardProps } from './types';
import { formatCompact, formatSigned, marginRateColor, marginBarColor } from '../components/utils';

export const AccountCard = memo(
  ({ bot, accountMetrics, totalPnl, totalPnlPct }: AccountCardProps) => {
    const pnlClass = useMemo(
      () =>
        totalPnl > 0 ? 'text-success-text' : totalPnl < 0 ? 'text-danger-text' : 'text-on-surface',
      [totalPnl],
    );

    const unrealizedPnl = accountMetrics.balance - bot.initial_capital - bot.total_pnl;
    const unrealizedClass =
      unrealizedPnl > 0
        ? 'text-success-text'
        : unrealizedPnl < 0
          ? 'text-danger-text'
          : 'text-on-surface-tertiary';

    return (
      <div className="rounded-lg border border-line-default bg-surface-1 p-3 flex flex-col gap-2.5">
        <span className="text-xs font-semibold text-on-surface">账户概览</span>

        <div className="flex items-center justify-between">
          <span className="text-2xs text-on-surface-tertiary">账户余额</span>
          <span className="text-sm font-bold font-mono tabular-nums text-on-surface">
            {formatCompact(accountMetrics.balance)}
          </span>
        </div>

        <div className="flex flex-col gap-1">
          <div className="flex items-center justify-between">
            <span className="text-2xs text-on-surface-tertiary">已用保证金</span>
            <span
              className={`text-xs font-mono font-semibold tabular-nums ${marginRateColor(accountMetrics.marginRate)}`}
            >
              {formatCompact(accountMetrics.usedMargin)}
              <span className="text-on-surface-muted ml-1">
                ({accountMetrics.marginRate.toFixed(1)}%)
              </span>
            </span>
          </div>
          <div className="h-1 bg-surface-2 rounded-full overflow-hidden">
            <div
              className={`h-full rounded-full transition-all duration-700 ease-out ${marginBarColor(accountMetrics.marginRate)}`}
              style={{ width: `${Math.min(Math.max(accountMetrics.marginRate, 0), 100)}%` }}
            />
          </div>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-2xs text-on-surface-tertiary">保证金率</span>
          <span
            className={`text-xs font-mono font-semibold tabular-nums ${marginRateColor(accountMetrics.marginRate)}`}
          >
            {accountMetrics.marginRate.toFixed(2)}%
          </span>
        </div>

        <div className="flex items-center justify-between pt-1 border-t border-line-subtle">
          <span className="text-2xs text-on-surface-tertiary">总盈亏</span>
          <div className="flex items-baseline gap-1.5">
            <span className={`text-sm font-bold font-mono tabular-nums ${pnlClass}`}>
              {formatSigned(totalPnl)}
            </span>
            <span className={`text-2xs font-semibold font-mono tabular-nums ${pnlClass}`}>
              {totalPnlPct > 0 ? '+' : ''}
              {totalPnlPct.toFixed(2)}%
            </span>
          </div>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-2xs text-on-surface-tertiary">未实现盈亏</span>
          <span className={`text-xs font-mono font-semibold tabular-nums ${unrealizedClass}`}>
            {unrealizedPnl > 0 ? '+' : ''}
            {formatCompact(unrealizedPnl)}
          </span>
        </div>
      </div>
    );
  },
);
