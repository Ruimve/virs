import { memo, type ReactNode } from 'react';

interface ReviewRowProps {
  label: string;
  value: ReactNode;
  mono?: boolean;
}

export const ReviewRow = memo(({ label, value, mono }: ReviewRowProps) => (
  <div className="flex items-center justify-between px-4 py-2.5 border-b border-line-subtle last:border-b-0 gap-3">
    <span className="text-13 text-on-surface-tertiary shrink-0">{label}</span>
    <span className={`text-13 font-medium text-on-base text-right ${mono ? 'font-mono' : ''}`}>
      {value}
    </span>
  </div>
));
