import { memo, type ReactNode } from 'react';

export interface TitleProps {
  children: ReactNode;
  className?: string;
}

export const Title = memo(({ children, className = '' }: TitleProps) => (
  <div
    className={`text-[11px] uppercase tracking-[0.15em] text-on-surface-tertiary font-medium ${className}`}
  >
    {children}
  </div>
));
