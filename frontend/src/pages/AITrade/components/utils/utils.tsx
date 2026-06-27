import type { ReactNode } from 'react';

export const formatPnl = (pnl: number): ReactNode => {
  if (pnl > 0)
    return <span className="text-success-text font-semibold font-mono">+{pnl.toFixed(4)}</span>;
  if (pnl < 0)
    return <span className="text-danger-text font-semibold font-mono">{pnl.toFixed(4)}</span>;
  return <span className="text-on-surface-tertiary font-mono">0.00</span>;
};

export const formatPnlShort = (pnl: number): ReactNode => {
  if (pnl > 0)
    return <span className="text-success-text font-semibold font-mono">+{pnl.toFixed(2)}</span>;
  if (pnl < 0)
    return <span className="text-danger-text font-semibold font-mono">{pnl.toFixed(2)}</span>;
  return <span className="text-on-surface-tertiary font-mono">0.00</span>;
};

export const formatSmart = (value: number): string => {
  if (value === 0) return '0';
  if (value >= 1) return value.toFixed(4);
  if (value >= 0.001) return value.toFixed(6);
  if (value >= 0.000001) return value.toFixed(8);
  return value.toExponential(2);
};

export const formatTime = (date: string | Date | undefined | null): string => {
  if (!date) return '-';
  return new Date(date).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
  });
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
    adjust_grid: '调整网格',
    pause_grid: '暂停网格',
    resume_grid: '恢复网格',
  };
  return map[action] || action;
};

export const actionColor = (action: string | undefined) => {
  const defaultColor = 'bg-surface-2 text-on-surface-tertiary border-line-default';
  if (!action) return defaultColor;

  const map: Record<string, string> = {
    open_long: 'bg-success-bg text-success-text border-success-border',
    open_short: 'bg-danger-bg text-danger-text border-danger-border',
    close_position: 'bg-info-bg text-info-text border-info-border',
    hold: defaultColor,
    reduce_position: 'bg-warning-bg text-warning-text border-warning-border',
    cancel_order: defaultColor,
    adjust_grid: 'bg-info-bg text-info-text border-info-border',
    pause_grid: 'bg-danger-bg text-danger-text border-danger-border',
    resume_grid: 'bg-success-bg text-success-text border-success-border',
  };
  return map[action] || defaultColor;
};

export const confidenceColor = (conf: number) => {
  if (conf >= 0.7) return 'text-success-text';
  if (conf >= 0.4) return 'text-warning-text';
  return 'text-danger-text';
};

export const tradeTypeLabel = (t: string) => {
  const map: Record<string, string> = {
    open_long: '开多',
    open_short: '开空',
    close_long: '平多',
    close_short: '平空',
    stop_loss: '止损',
    take_profit: '止盈',
  };
  return map[t] || t;
};

export const tradeTypeColor = (t: string) => {
  if (t.startsWith('open_long') || t === 'close_short') return 'text-success-text';
  if (t.startsWith('open_short') || t === 'close_long') return 'text-danger-text';
  if (t === 'stop_loss') return 'text-danger-text';
  if (t === 'take_profit') return 'text-success-text';
  return 'text-on-surface-tertiary';
};
