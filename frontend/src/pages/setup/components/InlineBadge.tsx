import { memo } from 'react';
import { Spinner, Check } from '@/components/Icon';

export type BadgeState = 'idle' | 'verifying' | 'success' | 'error';

interface InlineBadgeProps {
  state: BadgeState;
  text?: string;
}

export const InlineBadge = memo(({ state, text }: InlineBadgeProps) => {
  if (state === 'idle' || !text) return null;

  const styles: Record<BadgeState, string> = {
    idle: '',
    verifying: 'bg-info-bg text-info-text border-info-border',
    success: 'bg-success-bg text-success-text border-success-border',
    error: 'bg-danger-bg text-danger-text border-danger-border',
  };

  return (
    <span
      className={`inline-flex items-center gap-1 text-caption font-medium px-2 py-0.5 rounded-md border whitespace-nowrap ${styles[state]}`}
    >
      {state === 'verifying' && <Spinner className="w-3 h-3" />}
      {state === 'success' && <Check className="w-3 h-3" strokeWidth={2.5} />}
      {text}
    </span>
  );
});
