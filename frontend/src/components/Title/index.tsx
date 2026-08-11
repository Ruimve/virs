import { memo, type ReactNode } from 'react';

export interface TitleProps {
  children: ReactNode;
  className?: string;
}

export const Title = memo(({ children, className = '' }: TitleProps) => (
  <div
    className={`text-caption uppercase tracking-caption text-on-surface-tertiary font-medium ${className}`}
  >
    {children}
  </div>
));
