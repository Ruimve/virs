import { memo, type InputHTMLAttributes } from 'react';
import { Spinner } from '@/components/Icon';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /** Show loading spinner at the right side and disable interaction */
  loading?: boolean;
  /** Full width on mobile, auto width on sm+ screens (default: true) */
  responsive?: boolean;
}

const baseStyles =
  'w-full bg-surface-2 border rounded-xl text-on-base placeholder-placeholder focus:outline-none focus:border-accent transition-all duration-200 disabled:opacity-60 px-4 py-2.5 text-sm';

export const Input = memo(
  ({ loading = false, responsive = true, disabled, className = '', ...props }: InputProps) => {
    const isDisabled = disabled || loading;
    const borderClass = loading ? 'border-accent-muted' : 'border-line-strong';
    const wrapperClass = responsive ? 'w-full sm:w-auto sm:inline-block' : 'w-full';

    return (
      <div className={`relative ${wrapperClass}`}>
        <input
          disabled={isDisabled}
          className={[baseStyles, borderClass, className].filter(Boolean).join(' ')}
          {...props}
        />
        {loading && (
          <Spinner className={`absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-accent`} />
        )}
      </div>
    );
  },
);
