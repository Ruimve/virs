import { memo, type ButtonHTMLAttributes, type ReactNode } from 'react';

export type IconBtnSize = 'sm' | 'md';

export interface IconBtnProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  size?: IconBtnSize;
}

const sizeClasses: Record<IconBtnSize, string> = {
  sm: 'p-1',
  md: 'p-1.5',
};

export const IconBtn = memo(({ children, size = 'md', className = '', ...props }: IconBtnProps) => (
  <button
    type="button"
    className={`inline-flex items-center justify-center rounded-lg text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2 transition-colors ${sizeClasses[size]} ${className}`}
    {...props}
  >
    {children}
  </button>
));
