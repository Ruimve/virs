import type { AutoBot } from '@/service/types';
import type { PositionWsEvent } from '@/service/ws';
import type { SideMetrics, AccountMetrics } from './types';

const EMPTY_METRICS: SideMetrics = {
  hasPosition: false,
  unrealizedPnl: 0,
  usedMargin: 0,
  unrealizedPct: 0,
  estimatedLiqPrice: null,
};

export function computeMetrics(
  position: PositionWsEvent | null,
  latestPrice: number,
  leverage: number,
): SideMetrics {
  if (!position || position.status === 'closed' || position.quantity <= 0) {
    return EMPTY_METRICS;
  }

  const dir = position.side === 'long' ? 1 : -1;
  const unrealizedPnl =
    latestPrice > 0 ? (latestPrice - position.entryPrice) * position.quantity * dir : 0;
  const usedMargin = (position.quantity * position.entryPrice) / leverage;
  const unrealizedPct = usedMargin > 0 ? (unrealizedPnl / usedMargin) * 100 : 0;

  const liqOffset = position.entryPrice / leverage;
  const estimatedLiqPrice =
    position.side === 'long' ? position.entryPrice - liqOffset : position.entryPrice + liqOffset;

  return { hasPosition: true, unrealizedPnl, usedMargin, unrealizedPct, estimatedLiqPrice };
}

export function computeAccount(
  bot: AutoBot,
  totalUnrealizedPnl: number,
  totalUsedMargin: number,
): AccountMetrics {
  const balance = bot.initial_capital + bot.total_pnl + totalUnrealizedPnl;
  const freeMargin = balance - totalUsedMargin;
  const marginRate = balance > 0 ? (totalUsedMargin / balance) * 100 : 0;
  return { balance, usedMargin: totalUsedMargin, freeMargin, marginRate };
}

export function computeTotalPnlPct(bot: AutoBot, totalPnl: number): number {
  return bot.initial_capital > 0 ? (totalPnl / bot.initial_capital) * 100 : 0;
}

export function computeLiqDistance(
  latestPrice: number,
  liqPrice: number | null,
  side: 'long' | 'short',
): number | null {
  if (!liqPrice || liqPrice <= 0 || latestPrice <= 0) return null;
  const dist =
    side === 'long'
      ? ((latestPrice - liqPrice) / latestPrice) * 100
      : ((liqPrice - latestPrice) / latestPrice) * 100;
  return dist;
}

export function formatCompact(n: number, decimals = 2): string {
  return n.toLocaleString('en-US', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

export function formatSigned(n: number, decimals = 2): string {
  if (n > 0) return '+' + formatCompact(n, decimals);
  if (n < 0) return formatCompact(n, decimals);
  return formatCompact(n, decimals);
}

export function formatRelativeTime(isoTime: string | undefined | null): string {
  if (!isoTime) return '';
  const ts = new Date(isoTime).getTime();
  if (isNaN(ts)) return '';
  const diff = Date.now() - ts;
  if (diff < 0) return '';
  const m = Math.floor(diff / 60000);
  if (m < 1) return '刚刚';
  if (m < 60) return `${m}m前`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h前`;
  const d = Math.floor(h / 24);
  return `${d}d前`;
}

export function formatDuration(isoTime: string | undefined | null): string {
  if (!isoTime) return '';
  const ts = new Date(isoTime).getTime();
  if (isNaN(ts)) return '';
  const diff = Date.now() - ts;
  if (diff < 0) return '';
  const m = Math.floor(diff / 60000);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  const d = Math.floor(h / 24);
  return `${d}d`;
}

export function formatInterval(secs: number): string {
  if (secs <= 0) return '';
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h`;
}

export function marginRateColor(rate: number): string {
  if (rate >= 80) return 'text-danger-text';
  if (rate >= 50) return 'text-warning-text';
  return 'text-on-surface-secondary';
}

export function marginBarColor(rate: number): string {
  if (rate >= 80) return 'bg-danger';
  if (rate >= 50) return 'bg-warning';
  return 'bg-success';
}
