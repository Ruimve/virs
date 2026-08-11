import { useState, useEffect, type ReactNode, memo, useMemo, useCallback } from 'react';
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

    const isEditable = useMemo(() => editable !== false && status === 'done', [editable, status]);
    const isCollapsed = useMemo(() => status === 'done' && !expanded, [status, expanded]);

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

    const renderIndicator = useCallback(() => {
      if (indicator === false) return null;

      const stepContent =
        typeof step === 'number' ? <span className="text-caption">{step}</span> : step;

      const INDICATOR_BASE = 'w-7 h-7 rounded-full flex items-center justify-center';

      switch (status) {
        case 'pending':
          return (
            <div
              className={`${INDICATOR_BASE} border border-line-strong bg-surface-1 text-on-surface-faint`}
            >
              {stepContent}
            </div>
          );
        case 'active':
          return (
            <div className={`${INDICATOR_BASE} bg-accent/80 text-white font-medium`}>
              {stepContent}
            </div>
          );
        case 'verifying':
          return (
            <div className={`${INDICATOR_BASE} bg-accent-muted border border-accent-muted`}>
              <Spinner className="w-3.5 h-3.5 text-accent" />
            </div>
          );
        case 'done':
          return (
            <div className={`${INDICATOR_BASE} bg-success-bg border border-success-border`}>
              <Check className="w-3.5 h-3.5 text-success-text" strokeWidth={2.5} />
            </div>
          );
        case 'error':
          return (
            <div className={`${INDICATOR_BASE} bg-danger-bg border border-danger-border`}>
              <Close className="w-3.5 h-3.5 text-danger-text" strokeWidth={2.5} />
            </div>
          );
      }
    }, [indicator, step, status]);

    const renderLine = useCallback(() => {
      const show = showLine !== undefined ? showLine : !isCollapsed;
      if (!show) return null;
      return <div className="w-px flex-1 min-h-4 bg-line-default mt-1" />;
    }, [showLine, isCollapsed]);

    const renderContent = useCallback(
      (slot: ReactNode) => {
        const show =
          status === 'active' ||
          status === 'verifying' ||
          status === 'error' ||
          (status === 'done' && expanded);
        if (!show) return null;
        return slot;
      },
      [status, expanded],
    );

    const titleColor = useMemo(() => {
      switch (status) {
        case 'pending':
          return 'text-on-surface-faint';
        case 'done':
          return 'text-on-surface-tertiary';
        default:
          return 'text-on-surface';
      }
    }, [status]);

    return (
      <div className="flex gap-3">
        <div className="flex flex-col items-center">
          <div
            className={isEditable ? 'cursor-pointer' : ''}
            onClick={handleHeaderClick}
            onKeyDown={handleHeaderKeyDown}
          >
            {renderIndicator()}
          </div>
          {renderLine()}
        </div>

        <div className="flex-1 pb-4">
          <div
            className={`flex items-center gap-2 ${isEditable ? 'cursor-pointer group' : ''}`}
            onClick={handleHeaderClick}
            onKeyDown={handleHeaderKeyDown}
          >
            <p className={`text-sm font-medium leading-7 ${titleColor}`}>{title}</p>
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

          {renderContent(children)}
        </div>
      </div>
    );
  },
);
