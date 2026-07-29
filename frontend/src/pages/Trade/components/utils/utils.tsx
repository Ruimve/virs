import type { ReactNode } from 'react';

export const formatPnl = (pnl: number): ReactNode => {
  if (pnl > 0)
    return <span className="text-success-text font-semibold font-mono">+{pnl.toFixed(4)}</span>;
  if (pnl < 0)
    return <span className="text-danger-text font-semibold font-mono">{pnl.toFixed(4)}</span>;
  return <span className="text-on-surface-tertiary font-mono">0.00</span>;
};

export const actionLabel = (action: string | undefined) => {
  if (!action) return undefined;

  const map: Record<string, string> = {
    open_long: '开多',
    open_short: '开空',
    close_position: '平仓',
    hold: '持有',
    reduce_position: '减仓',
    cancel_order: '取消订单',
  };
  return map[action] || action;
};

export const actionVariant = (
  action: string | undefined,
): 'success' | 'danger' | 'warning' | 'info' | 'neutral' => {
  if (!action) return 'neutral';
  const map: Record<string, 'success' | 'danger' | 'warning' | 'info' | 'neutral'> = {
    open_long: 'success',
    open_short: 'danger',
    close_position: 'info',
    hold: 'neutral',
    reduce_position: 'warning',
    cancel_order: 'neutral',
  };
  return map[action] || 'neutral';
};
