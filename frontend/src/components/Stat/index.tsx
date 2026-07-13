import { memo, type ReactNode } from 'react';

interface StatProps {
  label: string;
  children: ReactNode;
  highlight?: boolean; // if true, uses text-base font-semibold
  color?: string; // extra color class for value
  sub?: string; // optional sub text below value
}

export const Stat = memo(({ label, children, highlight, color, sub }: StatProps) => (
  <div className="min-w-0">
    <div className="text-[11px] uppercase tracking-wider text-on-surface-tertiary mb-0.5">
      {label}
    </div>
    <div
      className={`font-mono tabular-nums truncate ${highlight ? 'text-base font-semibold' : 'text-sm'} ${color || 'text-on-surface'}`}
    >
      {children}
    </div>
    {sub && <div className="text-[10px] text-on-surface-muted mt-0.5">{sub}</div>}
  </div>
));
