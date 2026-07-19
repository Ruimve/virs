import { memo, type ReactNode } from 'react';

export type BadgeVariant = 'success' | 'danger' | 'warning' | 'info' | 'neutral';
export type BadgeSize = 'xs' | 'sm';

export interface BadgeProps {
  variant: BadgeVariant;
  children: ReactNode;
  size?: BadgeSize;
  className?: string;
}

const variantClasses: Record<BadgeVariant, string> = {
  success: 'bg-success-bg text-success-text',
  danger: 'bg-danger-bg text-danger-text',
  warning: 'bg-warning-bg text-warning-text',
  info: 'bg-info-bg text-info-text',
  neutral: 'bg-surface-2 text-on-surface-tertiary',
};

const sizeClasses: Record<BadgeSize, string> = {
  xs: 'text-2xs px-1.5 py-0.5',
  sm: 'text-caption px-2 py-0.5',
};

export const Badge = memo(({ variant, children, size = 'xs', className = '' }: BadgeProps) => {
  return (
    <span
      className={`inline-block ${sizeClasses[size]} rounded font-medium shrink-0 ${variantClasses[variant]} ${className}`}
    >
      {children}
    </span>
  );
});
