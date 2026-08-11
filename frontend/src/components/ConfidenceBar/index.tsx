import { memo, useMemo } from 'react';

export interface ConfidenceBarProps {
  value: number;
  showValue?: boolean;
  className?: string;
}

export const ConfidenceBar = memo(
  ({ value, showValue = true, className = '' }: ConfidenceBarProps) => {
    const clampedValue = useMemo(() => Math.min(Math.max(value, 0), 100), [value]);

    return (
      <div className={`flex items-center gap-2 ${className}`}>
        <div className="flex-1 h-1.5 bg-surface-3 rounded-full overflow-hidden">
          <div
            className="h-full bg-ai rounded-full transition-all duration-500 ease-out"
            style={{ width: `${clampedValue}%` }}
          />
        </div>
        {showValue && (
          <span className="text-2xs font-mono tabular-nums text-on-surface-tertiary min-w-10 text-right">
            {clampedValue.toFixed(0)}%
          </span>
        )}
      </div>
    );
  },
);
