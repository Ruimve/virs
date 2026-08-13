import { memo, type ReactNode } from 'react';

export interface EmptyProps {
  text?: string;
  icon?: ReactNode;
  className?: string;
}

export const Empty = memo(({ text, icon, className = '' }: EmptyProps) => {
  return (
    <div
      className={`h-full flex flex-col items-center justify-center gap-3 text-on-surface-tertiary ${className}`}
    >
      {icon}
      {text && <span className="text-xs">{text}</span>}
    </div>
  );
});
