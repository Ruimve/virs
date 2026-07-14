import { memo, type InputHTMLAttributes } from 'react';
import { Spinner } from '@/components/Icon';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {

  loading?: boolean;
}

const baseStyles =
  'w-full bg-surface-2 border rounded-xl text-on-base placeholder-placeholder focus:outline-none focus:border-accent transition-all duration-200 disabled:opacity-60 px-4 py-2.5 text-sm';

export const Input = memo(({ loading = false, disabled, className = '', ...props }: InputProps) => {
  const isDisabled = disabled || loading;
  const borderClass = loading ? 'border-accent-muted' : 'border-line-strong';

  const loadingPad = loading ? 'pr-10' : '';

  return (
    <div className="relative w-full">
      <input
        disabled={isDisabled}
        className={[baseStyles, borderClass, loadingPad, className].filter(Boolean).join(' ')}
        {...props}
      />
      {loading && (
        <Spinner className={`absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-accent`} />
      )}
    </div>
  );
});
