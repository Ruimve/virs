import { memo } from 'react';

export interface ProgressBarProps {
  pct: number;
  size?: 'sm' | 'md';
  thresholds?: { warning: number; danger: number };
}

export const ProgressBar = memo(
  ({ pct, size = 'md', thresholds = { warning: 75, danger: 90 } }: ProgressBarProps) => {
    const barColor = () => {
      if (pct >= thresholds.danger) return 'from-danger/80 to-danger';
      if (pct >= thresholds.warning) return 'from-warning/80 to-warning';
      return 'from-success/80 to-success';
    };
    const heightClass = size === 'sm' ? 'h-1' : 'h-1.5';
    // Clamp to [0, 100] to handle negative or out-of-range values
    const clampedPct = Math.min(Math.max(pct, 0), 100);

    return (
      <div className={`${heightClass} bg-surface-2/80 rounded-full overflow-hidden`}>
        <div
          className={`h-full bg-linear-to-r ${barColor()} rounded-full transition-all duration-700 ease-out`}
          style={{ width: `${clampedPct}%` }}
        />
      </div>
    );
  },
);
