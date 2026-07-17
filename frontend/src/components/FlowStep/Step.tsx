import { useState, useEffect, type ReactNode, memo } from 'react';
import { Spinner, Check, Close } from '@/components/Icon';

export type FlowStepStatus = 'pending' | 'active' | 'verifying' | 'done' | 'error';

export interface FlowStepProps {
  step: number | ReactNode;
  title: string;
  description?: string;
  status: FlowStepStatus;
  summary?: string | ReactNode;
  editable?: boolean;
  showLine?: boolean;
  indicator?: ReactNode;
  onToggle?: (expanded: boolean) => void;
  children?: ReactNode;
}

export const FlowStep = memo(
  ({
    step,
    title,
    description,
    status,
    summary,
    editable,
    showLine,
    indicator,
    onToggle,
    children,
  }: FlowStepProps) => {
    const [expanded, setExpanded] = useState(false);

    useEffect(() => {
      if (status === 'active' || status === 'verifying' || status === 'error') {
        setExpanded(true);
      }
      if (status === 'done') {
        setExpanded(false);
      }
    }, [status]);

    const isEditable = editable !== false && status === 'done';
    const isCollapsed = status === 'done' && !expanded;
    const showContent =
      status === 'active' ||
      status === 'verifying' ||
      status === 'error' ||
      (status === 'done' && expanded);

    const handleHeaderClick = () => {
      if (!isEditable) return;
      const next = !expanded;
      setExpanded(next);
      onToggle?.(next);
    };

    const handleHeaderKeyDown = (e: React.KeyboardEvent) => {
      if (!isEditable) return;
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handleHeaderClick();
      }
    };

    const shouldShowLine = showLine !== undefined ? showLine : !isCollapsed;

    const defaultIndicator = (): ReactNode => {
      const stepContent =
        typeof step === 'number' ? <span className="text-caption">{step}</span> : step;

      switch (status) {
        case 'pending':
          return (
            <div className="w-7 h-7 rounded-full border border-line-strong bg-surface-1 flex items-center justify-center text-on-surface-faint">
              {stepContent}
            </div>
          );
        case 'active':
          return (
            <div className="w-7 h-7 rounded-full bg-accent/80 flex items-center justify-center text-white font-medium">
              {stepContent}
            </div>
          );
        case 'verifying':
          return (
            <div className="w-7 h-7 rounded-full bg-accent-muted border border-accent-muted flex items-center justify-center">
              <Spinner className="w-3.5 h-3.5 text-accent" />
            </div>
          );
        case 'done':
          return (
            <div className="w-7 h-7 rounded-full bg-success-bg border border-success-border flex items-center justify-center">
              <Check className="w-3.5 h-3.5 text-success-text" strokeWidth={2.5} />
            </div>
          );
        case 'error':
          return (
            <div className="w-7 h-7 rounded-full bg-danger-bg border border-danger-border flex items-center justify-center">
              <Close className="w-3.5 h-3.5 text-danger-text" strokeWidth={2.5} />
            </div>
          );
      }
    };

    const titleColor = () => {
      switch (status) {
        case 'pending':
          return 'text-on-surface-faint';
        case 'done':
          return 'text-on-surface-tertiary';
        default:
          return 'text-on-surface';
      }
    };

    return (
      <div className="flex gap-3">
        <div className="flex flex-col items-center">
          <div
            className={isEditable ? 'cursor-pointer' : ''}
            onClick={handleHeaderClick}
            onKeyDown={handleHeaderKeyDown}
          >
            {indicator ?? defaultIndicator()}
          </div>
          {shouldShowLine && <div className="w-px flex-1 min-h-[16px] bg-line-default mt-1" />}
        </div>

        <div className="flex-1 pb-4">
          <div
            className={`flex items-center gap-2 ${isEditable ? 'cursor-pointer group' : ''}`}
            onClick={handleHeaderClick}
            onKeyDown={handleHeaderKeyDown}
          >
            <p className={`text-sm font-medium leading-7 ${titleColor()}`}>{title}</p>
            {isEditable && (
              <span className="text-2xs text-on-surface-muted group-hover:text-on-surface-tertiary transition-colors">
                {expanded ? 'collapse' : 'edit'}
              </span>
            )}
          </div>

          {description && (status !== 'done' || expanded) && (
            <p className="text-xs text-on-surface-muted -mt-1 mb-1">{description}</p>
          )}

          {isCollapsed && summary && (
            <div className="text-xs text-on-surface-muted -mt-1">{summary}</div>
          )}

          {showContent && <div className="mt-2">{children}</div>}
        </div>
      </div>
    );
  },
);
