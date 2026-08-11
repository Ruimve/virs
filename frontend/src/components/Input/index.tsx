import { memo, useState, type InputHTMLAttributes, type ReactNode } from 'react';
import { Spinner, Eye, EyeOff } from '@/components/Icon';

type NativeInputProps = Omit<InputHTMLAttributes<HTMLInputElement>, 'prefix' | 'suffix'>;

export interface InputProps extends NativeInputProps {
  mono?: boolean;
  loading?: boolean;
  prefix?: ReactNode;
  suffix?: ReactNode;
}

const baseStyles =
  'w-full bg-surface-2 border rounded-lg text-on-base placeholder-placeholder text-sm py-2.5 outline-none transition-all duration-200 focus:border-accent focus:ring-2 focus:ring-accent-light disabled:opacity-60';

export const Input = memo(
  ({
    prefix,
    suffix,
    mono = false,
    loading = false,
    disabled,
    className = '',
    type = 'text',
    ...props
  }: InputProps) => {
    const [visible, setVisible] = useState(false);
    const toggleVisibility = type === 'password';

    const inputType = toggleVisibility ? (visible ? 'text' : 'password') : type;

    const borderClass = loading ? 'border-accent-muted' : 'border-line-strong';
    const padLeft = prefix ? 'pl-10' : 'pl-3.5';
    const padRight = toggleVisibility || suffix || loading ? 'pr-10' : 'pr-3.5';
    const monoClass = mono ? 'font-mono' : '';

    const classText = [baseStyles, borderClass, padLeft, padRight, monoClass, className]
      .filter(Boolean)
      .join(' ');

    return (
      <div className="relative w-full">
        {prefix && (
          <span className="absolute left-3 top-1/2 -translate-y-1/2 w-4.5 h-4.5 flex items-center justify-center text-on-surface-muted pointer-events-none">
            {prefix}
          </span>
        )}
        <input type={inputType} disabled={disabled || loading} className={classText} {...props} />
        {toggleVisibility && (
          <button
            type="button"
            onClick={() => setVisible((v) => !v)}
            aria-label={visible ? 'Hide' : 'Show'}
            className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center justify-center w-7 h-7 text-on-surface-muted hover:text-on-surface-secondary transition-colors"
          >
            {visible ? (
              <EyeOff className="w-4.5 h-4.5" strokeWidth={1.5} />
            ) : (
              <Eye className="w-4.5 h-4.5" strokeWidth={1.5} />
            )}
          </button>
        )}
        {suffix && !toggleVisibility && (
          <span className="absolute right-3.5 top-1/2 -translate-y-1/2 font-mono text-xs text-on-surface-muted pointer-events-none">
            {suffix}
          </span>
        )}
        {loading && (
          <Spinner className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-accent" />
        )}
      </div>
    );
  },
);
