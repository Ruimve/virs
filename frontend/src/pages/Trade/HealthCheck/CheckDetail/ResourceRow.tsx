import { memo } from 'react';
import MiniBar from './MiniBar';
import { usageColor } from '@/components/Progress/utils';

interface Props {
  label: string;
  pct: number;
  hint?: string;
}

const ResourceRow = ({ label, pct, hint }: Props) => (
  <div>
    <div className="flex items-baseline justify-between mb-1">
      <span className="text-caption text-on-surface-tertiary">{label}</span>
      <span className={`text-caption font-mono font-medium ${usageColor(pct)}`}>
        {pct.toFixed(0)}%
      </span>
    </div>
    <MiniBar pct={pct} />
    {hint && <span className="text-2xs text-on-surface-muted mt-0.5 block">{hint}</span>}
  </div>
);

export default memo(ResourceRow);
