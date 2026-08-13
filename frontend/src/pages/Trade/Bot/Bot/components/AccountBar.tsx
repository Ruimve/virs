import { memo } from 'react';
import type { AccountBarProps } from './types';
import { formatCompact, marginRateColor } from './utils';

export const AccountBar = memo(({ metrics }: AccountBarProps) => {
  const items = [
    {
      label: '权益',
      value: formatCompact(metrics.balance),
      valueClass: 'text-on-surface-secondary',
    },
    {
      label: '已用',
      value: formatCompact(metrics.usedMargin),
      valueClass: 'text-on-surface-secondary',
    },
    {
      label: '可用',
      value: formatCompact(metrics.freeMargin),
      valueClass: 'text-on-surface-secondary',
    },
    {
      label: '保证金率',
      value: `${metrics.marginRate.toFixed(2)}%`,
      valueClass: marginRateColor(metrics.marginRate),
    },
  ];

  return (
    <div className="flex items-center gap-0 py-1.5 border-t border-b border-line-subtle">
      {items.map((item, i) => (
        <div
          key={item.label}
          className={`flex items-baseline gap-1 px-2 ${i < items.length - 1 ? 'border-r border-line-subtle' : ''} ${i === 0 ? 'pl-0' : ''}`}
        >
          <span className="text-2xs text-on-surface-tertiary">{item.label}</span>
          <span className={`text-xs font-semibold font-mono tabular-nums ${item.valueClass}`}>
            {item.value}
          </span>
        </div>
      ))}
    </div>
  );
});
