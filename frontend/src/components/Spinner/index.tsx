import { memo, type ReactNode } from 'react';
import { Spinner as SpinnerIcon } from '@/components/Icon';

export interface SpinnerProps {
  text?: string;
  icon?: ReactNode;
  className?: string;
}

export const Spinner = memo(({ text, icon, className = '' }: SpinnerProps) => {
  return (
    <div
      className={`h-full flex flex-col items-center justify-center gap-4 text-on-surface-tertiary ${className}`}
    >
      {icon || <SpinnerIcon className="h-6 w-6" />}
      {text && <span className="text-xs tracking-wider">{text}</span>}
    </div>
  );
});
