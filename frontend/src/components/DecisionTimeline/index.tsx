import { memo } from 'react';

export type DecisionAction = 'buy' | 'sell' | 'hold';
export type DecisionResultType = 'up' | 'down' | 'pending';

export interface DecisionTimelineItem {
  time: string;
  action: DecisionAction;
  confidence: number;
  result?: string;
  resultType?: DecisionResultType;
}

export interface DecisionTimelineProps {
  items: DecisionTimelineItem[];
  className?: string;
}

const actionConfig: Record<DecisionAction, { dot: string; label: string; text: string }> = {
  buy: {
    dot: 'bg-success-bg border-success-border',
    label: 'text-success-text',
    text: '买入',
  },
  sell: {
    dot: 'bg-danger-bg border-danger-border',
    label: 'text-danger-text',
    text: '卖出',
  },
  hold: {
    dot: 'bg-surface-3 border-line-strong',
    label: 'text-on-surface-tertiary',
    text: '持有',
  },
};

const resultConfig: Record<DecisionResultType, string> = {
  up: 'text-success-text',
  down: 'text-danger-text',
  pending: 'text-on-surface-muted',
};

export const DecisionTimeline = memo(({ items, className = '' }: DecisionTimelineProps) => {
  return (
    <div className={`flex flex-col ${className}`}>
      {items.map((item, index) => {
        const config = actionConfig[item.action];
        const isLast = index === items.length - 1;

        return (
          <div key={`${item.time}-${item.action}-${index}`} className="flex gap-3">
            {/* Timeline column */}
            <div className="flex flex-col items-center">
              <div className={`w-2 h-2 rounded-full border ${config.dot} shrink-0 mt-1`} />
              {!isLast && <div className="w-px flex-1 bg-line-default min-h-5" />}
            </div>
            {/* Content */}
            <div className={`flex-1 ${isLast ? 'pb-0' : 'pb-3'}`}>
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <span className={`text-xs font-medium ${config.label}`}>{config.text}</span>
                  <span className="text-2xs text-on-surface-muted font-mono tabular-nums">
                    {item.time}
                  </span>
                </div>
                <div className="flex items-center gap-3">
                  {/* Confidence mini bar */}
                  <div className="flex items-center gap-1.5">
                    <div className="w-8 h-0.5 bg-surface-3 rounded-full overflow-hidden">
                      <div
                        className="h-full bg-ai rounded-full"
                        style={{ width: `${Math.min(Math.max(item.confidence, 0), 100)}%` }}
                      />
                    </div>
                    <span className="text-2xs font-mono tabular-nums text-on-surface-tertiary">
                      {item.confidence}%
                    </span>
                  </div>
                  {/* Result */}
                  {item.result && (
                    <span
                      className={`text-2xs font-mono font-semibold tabular-nums ${
                        item.resultType ? resultConfig[item.resultType] : 'text-on-surface-tertiary'
                      }`}
                    >
                      {item.result}
                    </span>
                  )}
                </div>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
});
