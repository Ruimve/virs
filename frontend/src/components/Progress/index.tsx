import { memo, useMemo } from 'react';

export interface ProgressProps {
  pct: number;
  size?: 'sm' | 'md';
  thresholds?: { warning: number; danger: number };
}

const defaultThresholds = { warning: 75, danger: 90 };

export const Progress = memo(
  ({ pct, size = 'md', thresholds = defaultThresholds }: ProgressProps) => {
    const barColor = useMemo(() => {
      if (pct >= thresholds.danger) return 'from-danger/80 to-danger';
      if (pct >= thresholds.warning) return 'from-warning/80 to-warning';
      return 'from-success/80 to-success';
    }, [pct, thresholds.warning, thresholds.danger]);
    const heightClass = useMemo(() => (size === 'sm' ? 'h-1' : 'h-1.5'), [size]);

    const clampedPct = useMemo(() => Math.min(Math.max(pct, 0), 100), [pct]);

    return (
      <div className={`${heightClass} bg-surface-2/80 rounded-full overflow-hidden`}>
        <div
          className={`h-full bg-linear-to-r ${barColor} rounded-full transition-all duration-700 ease-out`}
          style={{ width: `${clampedPct}%` }}
        />
      </div>
    );
  },
);
