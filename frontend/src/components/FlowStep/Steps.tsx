import { memo, type ReactNode } from 'react';
import FlowStep, { type FlowStepStatus } from './Step';

export type { FlowStepStatus };

export interface FlowStepConfig {
  key: string;
  title: string;
  description?: string;
  editable?: boolean;
  showLine?: boolean;
  indicator?: ReactNode;
  step?: number;
  render?: () => ReactNode;
}

export interface FlowStepsProps {
  steps: FlowStepConfig[];
  statuses: Record<string, FlowStepStatus>;
  summaries?: Record<string, string | ReactNode>;
  onToggles?: Record<string, (expanded: boolean) => void>;
}

function FlowSteps({ steps, statuses, summaries, onToggles }: FlowStepsProps) {
  return (
    <div className="space-y-1">
      {steps.map((config, index) => {
        const status = statuses[config.key] ?? 'pending';
        const summary = summaries?.[config.key];
        const onToggle = onToggles?.[config.key];

        return (
          <FlowStep
            key={config.key}
            step={config.step ?? index + 1}
            title={config.title}
            description={config.description}
            status={status}
            summary={summary}
            editable={config.editable}
            showLine={config.showLine}
            indicator={config.indicator}
            onToggle={onToggle}
          >
            {config.render?.()}
          </FlowStep>
        );
      })}
    </div>
  );
}

export default memo(FlowSteps);
