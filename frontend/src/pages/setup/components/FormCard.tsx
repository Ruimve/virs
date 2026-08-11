import { memo, type ReactNode } from 'react';

interface FormCardProps {
  children: ReactNode;
  className?: string;
}

export const FormCard = memo(({ children, className = '' }: FormCardProps) => (
  <div
    className={`border border-line-default rounded-xl bg-surface-1 overflow-hidden ${className}`}
  >
    {children}
  </div>
));
