import { memo, type ReactNode } from 'react';

interface SectionTitleProps {
  children: ReactNode;
  className?: string;
}

export const SectionTitle = memo(({ children, className = '' }: SectionTitleProps) => (
  <div
    className={`text-[11px] uppercase tracking-wider text-on-surface-tertiary font-medium ${className}`}
  >
    {children}
  </div>
));
