import type { ReactNode } from 'react';
export type { KlineCandle, OrderBookEntry, AnalysisLog, BotHeaderInfo as BotInfo } from '../../../service/types';

// ── Shared types (local to ai-trade components) ──────────

export type TabConfig = { key: string; label: string };

export interface StatusStyle {
  text: string;
  dot: string;
  bg: string;
}

// ── Shared utilities ──────────────────────────────────────

export function formatPnl(pnl: number): ReactNode {
  if (pnl > 0) return <span className="text-emerald-400 font-semibold">+{pnl.toFixed(4)}</span>;
  if (pnl < 0) return <span className="text-red-400 font-semibold">{pnl.toFixed(4)}</span>;
  return <span className="text-on-surface-tertiary">0.00</span>;
}

export function formatPnlShort(pnl: number): ReactNode {
  if (pnl > 0) return <span className="text-emerald-400 font-semibold">+{pnl.toFixed(2)}</span>;
  if (pnl < 0) return <span className="text-red-400 font-semibold">{pnl.toFixed(2)}</span>;
  return <span className="text-on-surface-tertiary">0.00</span>;
}

export function statusConfig(status: string): StatusStyle {
  const map: Record<string, StatusStyle> = {
    running: { text: '运行中', dot: 'bg-emerald-500', bg: 'bg-emerald-500/10 text-emerald-400' },
    paused: { text: '已暂停', dot: 'bg-amber-500', bg: 'bg-amber-500/10 text-amber-400' },
    stopped: { text: '已停止', dot: 'bg-on-surface-muted', bg: 'bg-surface-2 text-on-surface-tertiary' },
    draft: { text: '草稿', dot: 'bg-on-surface-faint', bg: 'bg-surface-2 text-on-surface-tertiary' },
    error: { text: '错误', dot: 'bg-red-500', bg: 'bg-red-500/10 text-red-400' },
  };
  return map[status] || { text: status, dot: 'bg-on-surface-muted', bg: 'bg-surface-2 text-on-surface-tertiary' };
}

export function formatSmart(value: number): string {
  if (value === 0) return '0';
  if (value >= 1) return value.toFixed(4);
  if (value >= 0.001) return value.toFixed(6);
  if (value >= 0.000001) return value.toFixed(8);
  return value.toExponential(2);
}
