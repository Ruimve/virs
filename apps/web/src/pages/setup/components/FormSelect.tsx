import { memo, type SelectHTMLAttributes, type ReactNode } from 'react';

interface FormSelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  children: ReactNode;
}

export const FormSelect = memo(({ children, className = '', ...props }: FormSelectProps) => (
  <div className="relative w-full">
    <select
      className={`w-full bg-surface-2 border border-line-strong rounded-lg text-on-base text-sm py-2.5 pl-3.5 pr-9.5 outline-none transition-all focus:border-accent focus:ring-2 focus:ring-accent-light appearance-none cursor-pointer ${className}`}
      {...props}
    >
      {children}
    </select>
    <svg
      className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-on-surface-muted pointer-events-none"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
    >
      <path d="M6 9l6 6 6-6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  </div>
));
