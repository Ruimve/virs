import { memo, type ReactNode } from 'react';

export interface SectionTitleProps {
  children: ReactNode;
  className?: string;
}

export const SectionTitle = memo(({ children, className = '' }: SectionTitleProps) => (
  <div
    className={`text-[11px] uppercase tracking-[0.15em] text-on-surface-tertiary font-medium ${className}`}
  >
    {children}
  </div>
));
