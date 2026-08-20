import { memo, useMemo, type ButtonHTMLAttributes, type ReactNode } from 'react';
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
  primary: 'bg-accent hover:bg-accent-hover text-white font-medium',
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
    const sizeStyle = useMemo(() => sizeStyles[size], [size]);
    const classText = useMemo(() => {
      const widthClass = responsive ? 'w-full sm:w-auto' : '';
      return [baseStyles, widthClass, sizeStyle.box, variantStyles[variant], className]
        .filter(Boolean)
        .join(' ');
    }, [responsive, sizeStyle.box, variant, className]);

    return (
      <button type={type} disabled={disabled || loading} className={classText} {...props}>
        {loading && <Spinner className={`${sizeStyle.spinner} shrink-0`} />}
        {children}
      </button>
    );
  },
);
