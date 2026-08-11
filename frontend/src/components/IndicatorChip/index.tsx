import { memo } from 'react';

export type IndicatorSentiment = 'bullish' | 'neutral' | 'bearish';

export interface IndicatorChipProps {
  name: string;
  value: string;
  sentiment: IndicatorSentiment;
  className?: string;
}

const sentimentClasses: Record<IndicatorSentiment, { container: string; value: string }> = {
  bullish: {
    container: 'bg-ai-bg border-ai-border',
    value: 'text-ai',
  },
  neutral: {
    container: 'bg-surface-2 border-line-default',
    value: 'text-on-surface-tertiary',
  },
  bearish: {
    container: 'bg-danger-bg border-danger-border',
    value: 'text-danger-text',
  },
};

export const IndicatorChip = memo(
  ({ name, value, sentiment, className = '' }: IndicatorChipProps) => {
    const styles = sentimentClasses[sentiment];

    return (
      <div
        className={`inline-flex items-center gap-1.5 px-2 py-1 rounded-md border ${styles.container} ${className}`}
      >
        <span className="text-2xs text-on-surface-muted">{name}</span>
        <span className={`text-2xs font-mono font-semibold tabular-nums ${styles.value}`}>
          {value}
        </span>
      </div>
    );
  },
);
