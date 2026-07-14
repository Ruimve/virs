export const usageColor = (pct: number, thresholds = { warning: 75, danger: 90 }): string => {
  if (pct >= thresholds.danger) return 'text-danger-text';
  if (pct >= thresholds.warning) return 'text-warning-text';
  return 'text-success-text';
};
