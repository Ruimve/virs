import { memo, useMemo } from 'react';
import type { RiskPanelProps } from './types';
import { formatCompact } from '../components/utils';

interface RiskMetric {
  label: string;
  value: string;
  pct: number;
  barClass: string;
  textClass: string;
}

function riskLevel(marginRate: number): { label: string; className: string } {
  if (marginRate >= 80)
    return { label: '高风险', className: 'bg-danger-bg text-danger-text border-danger-border' };
  if (marginRate >= 50)
    return { label: '中风险', className: 'bg-warning-bg text-warning-text border-warning-border' };
  return { label: '低风险', className: 'bg-success-bg text-success-text border-success-border' };
}

function MetricRow({ metric }: { metric: RiskMetric }) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between">
        <span className="text-2xs text-on-surface-tertiary">{metric.label}</span>
        <span className={`text-xs font-mono font-semibold tabular-nums ${metric.textClass}`}>
          {metric.value}
        </span>
      </div>
      <div className="h-1 bg-surface-2 rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-700 ease-out ${metric.barClass}`}
          style={{ width: `${Math.min(Math.max(metric.pct, 0), 100)}%` }}
        />
      </div>
    </div>
  );
}

export const RiskPanel = memo(
  ({ accountMetrics, bot, longMetrics, shortMetrics }: RiskPanelProps) => {
    const level = useMemo(() => riskLevel(accountMetrics.marginRate), [accountMetrics.marginRate]);

    const totalExposure = longMetrics.usedMargin + shortMetrics.usedMargin;
    const exposurePct =
      accountMetrics.balance > 0 ? (totalExposure / accountMetrics.balance) * 100 : 0;

    const totalLeverage = accountMetrics.balance > 0 ? totalExposure / accountMetrics.balance : 0;
    const leveragePct = Math.min((totalLeverage / Math.max(bot.leverage, 1)) * 100, 100);

    const maxDrawdown =
      Math.abs(bot.total_pnl) > 0 && bot.initial_capital > 0
        ? Math.min((Math.abs(bot.total_pnl) / bot.initial_capital) * 100, 100)
        : 0;

    const marginHealth = Math.max(100 - accountMetrics.marginRate, 0);

    const metrics: RiskMetric[] = useMemo(
      () => [
        {
          label: '总敞口',
          value: formatCompact(totalExposure),
          pct: exposurePct,
          barClass: 'bg-warning',
          textClass: 'text-warning-text',
        },
        {
          label: '总杠杆',
          value: `${totalLeverage.toFixed(2)}x`,
          pct: leveragePct,
          barClass: 'bg-accent',
          textClass: 'text-accent',
        },
        {
          label: '最大回撤',
          value: `${maxDrawdown.toFixed(1)}%`,
          pct: maxDrawdown,
          barClass: 'bg-danger',
          textClass: 'text-danger-text',
        },
        {
          label: '保证金充足率',
          value: `${marginHealth.toFixed(1)}%`,
          pct: marginHealth,
          barClass: 'bg-success',
          textClass: 'text-success-text',
        },
      ],
      [totalExposure, exposurePct, totalLeverage, leveragePct, maxDrawdown, marginHealth],
    );

    return (
      <div className="rounded-lg border border-line-default bg-surface-1 p-3 flex flex-col gap-2.5">
        {}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-semibold text-on-surface">
              <span className="text-warning-text">{'\u26A1'}</span> 风控面板
            </span>
          </div>
          <span
            className={`inline-flex items-center px-1.5 py-0.5 rounded text-2xs font-medium border ${level.className}`}
          >
            {level.label}
          </span>
        </div>

        {}
        <div className="flex flex-col gap-2">
          {metrics.map((m) => (
            <MetricRow key={m.label} metric={m} />
          ))}
        </div>
      </div>
    );
  },
);
