import { memo, type ReactNode } from 'react';

interface StatProps {
  label: string;
  children: ReactNode;
  highlight?: boolean;
  color?: string;
  sub?: string;
}

export const Stat = memo(({ label, children, highlight, color, sub }: StatProps) => (
  <div className="min-w-0">
    <div className="text-caption uppercase tracking-wider text-on-surface-tertiary mb-0.5">
      {label}
    </div>
    <div
      className={`font-mono tabular-nums truncate ${highlight ? 'text-base font-semibold' : 'text-sm'} ${color || 'text-on-surface'}`}
    >
      {children}
    </div>
    {sub && <div className="text-2xs text-on-surface-muted mt-0.5">{sub}</div>}
  </div>
));
