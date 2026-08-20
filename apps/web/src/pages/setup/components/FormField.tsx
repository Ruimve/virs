import { memo, type ReactNode } from 'react';

interface FormFieldProps {
  label: string;
  required?: boolean;
  badge?: ReactNode;
  children: ReactNode;
  hint?: string;
  noBorder?: boolean;
}

export const FormField = memo(
  ({ label, required, badge, children, hint, noBorder }: FormFieldProps) => (
    <div
      className={`px-4 py-3.5 ${!noBorder ? 'border-b border-line-subtle last:border-b-0' : ''}`}
    >
      {(label || badge) && (
        <div className="flex items-center justify-between mb-2 gap-2">
          {label && (
            <span className="text-13 font-medium text-on-surface-secondary">
              {label}
              {required && <span className="text-accent ml-0.5">*</span>}
            </span>
          )}
          {badge}
        </div>
      )}
      {children}
      {hint && <p className="text-caption text-on-surface-muted mt-1.5 leading-relaxed">{hint}</p>}
    </div>
  ),
);
