import { memo, type ReactNode } from 'react';

export interface KeyValueRowProps {
  label: string;
  value: ReactNode;
  valueColor?: string;
  border?: boolean;
}

export const KeyValueRow = memo(({ label, value, valueColor, border = true }: KeyValueRowProps) => (
  <div
    className={`flex items-center justify-between px-4 py-3 ${border ? 'border-b border-line-subtle/50' : ''}`}
  >
    <span className="text-[12px] text-on-surface-tertiary">{label}</span>
    <span
      className={`text-[12px] font-mono tabular-nums ${valueColor || 'text-on-surface-secondary'}`}
    >
      {value}
    </span>
  </div>
));
