import { memo, type ReactNode } from 'react';

type BadgeVariant = 'success' | 'danger' | 'warning' | 'info' | 'neutral';

interface BadgeProps {
  variant: BadgeVariant;
  children: ReactNode;
  size?: 'xs' | 'sm';
  className?: string;
}

const variantClasses: Record<BadgeVariant, string> = {
  success: 'bg-success-bg text-success-text',
  danger: 'bg-danger-bg text-danger-text',
  warning: 'bg-warning-bg text-warning-text',
  info: 'bg-info-bg text-info-text',
  neutral: 'bg-surface-2 text-on-surface-tertiary',
};

const Badge = memo(({ variant, children, size = 'xs', className = '' }: BadgeProps) => {
  const sizeClass = size === 'sm' ? 'text-[11px] px-2 py-0.5' : 'text-[10px] px-1.5 py-0.5';
  return (
    <span
      className={`inline-block ${sizeClass} rounded font-medium shrink-0 ${variantClasses[variant]} ${className}`}
    >
      {children}
    </span>
  );
});

export default Badge;
export type { BadgeVariant, BadgeProps };
