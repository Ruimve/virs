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

export const executionStatusLabel = (status: string | null | undefined) => {
  if (!status) return undefined;
  const map: Record<string, string> = {
    open: '开仓成功',
    open_failed: '开仓失败',
    open_canceled: '开仓取消',
    close: '平仓成功',
    close_failed: '平仓失败',
    close_canceled: '平仓取消',
    hold: '观望',
  };
  return map[status] || status;
};

export const executionStatusVariant = (
  status: string | null | undefined,
): 'success' | 'danger' | 'warning' | 'info' | 'neutral' => {
  if (!status) return 'neutral';
  const map: Record<string, 'success' | 'danger' | 'warning' | 'info' | 'neutral'> = {
    open: 'success',
    open_failed: 'danger',
    open_canceled: 'warning',
    close: 'info',
    close_failed: 'danger',
    close_canceled: 'warning',
    hold: 'neutral',
  };
  return map[status] || 'neutral';
};
