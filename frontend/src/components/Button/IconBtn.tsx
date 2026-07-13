import { memo, type ButtonHTMLAttributes, type ReactNode } from 'react';

export interface IconBtnProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  /** Size preset (default: md) */
  size?: 'sm' | 'md';
}

const sizeStyles = {
  sm: 'p-1',
  md: 'p-1.5',
};

/**
 * Minimal icon-only button for toolbars and nav headers.
 * Renders a <button> for accessibility but visually compact.
 */
export const IconBtn = memo(({ children, size = 'md', className = '', ...props }: IconBtnProps) => (
  <button
    type="button"
    className={`inline-flex items-center justify-center rounded-lg text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2 transition-colors ${sizeStyles[size]} ${className}`}
    {...props}
  >
    {children}
  </button>
));
