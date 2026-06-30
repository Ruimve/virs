import { memo } from 'react';
import MiniBar from './MiniBar';

const pctColor = (pct: number): string => {
  if (pct >= 90) return 'text-danger-text';
  if (pct >= 75) return 'text-warning-text';
  return 'text-success-text';
};

interface Props {
  label: string;
  pct: number;
  hint?: string;
}

const ResourceRow = ({ label, pct, hint }: Props) => (
  <div>
    <div className="flex items-baseline justify-between mb-1">
      <span className="text-[11px] text-on-surface-tertiary">{label}</span>
      <span className={`text-[11px] font-mono font-medium ${pctColor(pct)}`}>
        {pct.toFixed(0)}%
      </span>
    </div>
    <MiniBar pct={pct} />
    {hint && <span className="text-[10px] text-on-surface-muted mt-0.5 block">{hint}</span>}
  </div>
);

export default memo(ResourceRow);
