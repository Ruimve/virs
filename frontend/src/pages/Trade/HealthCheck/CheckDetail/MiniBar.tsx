import { memo } from 'react';

const pctBar = (pct: number): string => {
  if (pct >= 90) return 'bg-danger';
  if (pct >= 75) return 'bg-warning';
  return 'bg-success';
};

const MiniBar = ({ pct }: { pct: number }) => (
  <div className="h-1 w-full bg-surface-2 rounded-full overflow-hidden">
    <div
      className={`h-full ${pctBar(pct)} rounded-full transition-all duration-500`}
      style={{ width: `${Math.min(pct, 100)}%` }}
    />
  </div>
);

export default memo(MiniBar);
