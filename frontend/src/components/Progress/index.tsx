import { memo, useMemo } from 'react';

export type ProgressColor = 'threshold' | 'ai' | 'success' | 'danger' | 'warning';

export interface ProgressProps {
  pct: number;
  size?: 'sm' | 'md';
  color?: ProgressColor;
  showValue?: boolean;
  thresholds?: { warning: number; danger: number };
  className?: string;
}

const defaultThresholds = { warning: 75, danger: 90 };

const solidFill: Record<Exclude<ProgressColor, 'threshold'>, string> = {
  ai: 'bg-ai',
  success: 'bg-success',
  danger: 'bg-danger',
  warning: 'bg-warning',
};

export const Progress = memo(
  ({
    pct,
    size = 'md',
    color = 'threshold',
    showValue = false,
    thresholds = defaultThresholds,
    className = '',
  }: ProgressProps) => {
    const clampedPct = useMemo(() => Math.min(Math.max(pct, 0), 100), [pct]);
    const heightClass = useMemo(() => (size === 'sm' ? 'h-1' : 'h-1.5'), [size]);

    const fillClass = useMemo(() => {
      if (color === 'threshold') {
        if (pct >= thresholds.danger) return 'bg-linear-to-r from-danger/80 to-danger';
        if (pct >= thresholds.warning) return 'bg-linear-to-r from-warning/80 to-warning';
        return 'bg-linear-to-r from-success/80 to-success';
      }
      return solidFill[color];
    }, [color, pct, thresholds.warning, thresholds.danger]);

    return (
      <div className={`flex items-center gap-2 ${className}`}>
        <div className={`flex-1 ${heightClass} bg-surface-3 rounded-full overflow-hidden`}>
          <div
            className={`h-full ${fillClass} rounded-full transition-all duration-500 ease-out`}
            style={{ width: `${clampedPct}%` }}
          />
        </div>
        {showValue && (
          <span className="text-2xs font-mono tabular-nums text-on-surface-tertiary min-w-10 text-right">
            {clampedPct.toFixed(0)}%
          </span>
        )}
      </div>
    );
  },
);
