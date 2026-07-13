import { memo, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { Spinner } from '@/components/Icon';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'error';
export type ButtonSize = 'small' | 'normal';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /**
   * Preset style variant:
   * - primary: blue accent background with white text (default)
   * - secondary: light surface background with border
   * - ghost: transparent background, text-only
   * - error: danger red background with white text
   */
  variant?: ButtonVariant;
  /**
   * Button size preset:
   * - normal: standard padding & text (default)
   * - small: compact padding & smaller text
   */
  size?: ButtonSize;
  /** Show loading spinner and disable interaction */
  loading?: boolean;
  /** Full width on mobile, auto on sm+ screens (default: true) */
  responsive?: boolean;
  children?: ReactNode;
}

const baseStyles =
  'inline-flex items-center justify-center gap-2 rounded-xl transition-all duration-200 disabled:opacity-30 disabled:cursor-not-allowed';

const variantStyles: Record<ButtonVariant, string> = {
  primary: 'bg-accent/80 hover:bg-accent-hover text-white font-medium',
  secondary: 'bg-surface-2 hover:bg-surface-3 text-on-surface-secondary border border-line-default',
  ghost: 'text-on-surface-tertiary hover:text-on-surface-secondary',
  error: 'bg-danger/80 hover:bg-danger text-white font-medium',
};

const sizeStyles: Record<ButtonSize, { box: string; spinner: string }> = {
  normal: { box: 'px-6 py-2.5 text-sm', spinner: 'w-4 h-4' },
  small: { box: 'px-3.5 py-1.5 text-xs', spinner: 'w-3.5 h-3.5' },
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
