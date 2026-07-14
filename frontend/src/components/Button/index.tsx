import { memo, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { Spinner } from '@/components/Icon';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'error' | 'accent-outline';
export type ButtonSize = 'xs' | 'small' | 'normal';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  responsive?: boolean;
  children?: ReactNode;
}

const baseStyles =
  'inline-flex items-center justify-center gap-2 rounded-lg transition-all duration-200 disabled:opacity-30 disabled:cursor-not-allowed select-none';

const variantStyles: Record<ButtonVariant, string> = {
  primary:
    'bg-accent hover:bg-accent-hover text-white font-medium shadow-sm shadow-accent/10 hover:shadow-accent/20',
  secondary: 'bg-surface-2 hover:bg-surface-3 text-on-surface-secondary border border-line-default',
  ghost: 'text-on-surface-tertiary hover:text-on-surface-secondary hover:bg-surface-2/50',
  error: 'bg-danger hover:bg-danger-hover text-white font-medium',
  'accent-outline':
    'bg-accent-light border border-accent-muted text-accent hover:bg-accent-muted font-medium',
};

const sizeStyles: Record<ButtonSize, { box: string; spinner: string }> = {
  xs: { box: 'px-2.5 py-1 text-caption', spinner: 'w-3 h-3' },
  small: { box: 'px-3.5 py-1.5 text-xs', spinner: 'w-3.5 h-3.5' },
  normal: { box: 'px-5 py-2.5 text-sm', spinner: 'w-4 h-4' },
};

export const Button = memo(
  ({
    variant = 'primary',
    size = 'normal',
    loading = false,
    responsive = true,
    disabled,
    className = '',
    type = 'button',
    children,
    ...props
  }: ButtonProps) => {
    const isDisabled = disabled || loading;
    const widthClass = responsive ? 'w-full sm:w-auto' : '';
    const { box: sizeBox, spinner: spinnerSize } = sizeStyles[size];

    return (
      <button
        type={type}
        disabled={isDisabled}
        className={[baseStyles, widthClass, sizeBox, variantStyles[variant], className]
          .filter(Boolean)
          .join(' ')}
        {...props}
      >
        {loading && <Spinner className={`${spinnerSize} shrink-0`} />}
        {children}
      </button>
    );
  },
);
