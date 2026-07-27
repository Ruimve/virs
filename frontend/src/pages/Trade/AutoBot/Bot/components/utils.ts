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

/**
 * 计算单边持仓指标。
 *
 * 未实现盈亏 = (latestPrice - entryPrice) * quantity * dir
 * 已用保证金 = quantity * entryPrice / leverage
 * 收益率     = unrealizedPnl / usedMargin * 100
 *
 * 强平价为粗略估算（后端 PositionWsEvent 不提供 liquidation_price）：
 *   多头 ≈ entry × (1 - 1/leverage)
 *   空头 ≈ entry × (1 + 1/leverage)
 */
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

  // 粗略强平价估算
  const liqOffset = position.entryPrice / leverage;
  const estimatedLiqPrice =
    position.side === 'long' ? position.entryPrice - liqOffset : position.entryPrice + liqOffset;

  return { hasPosition: true, unrealizedPnl, usedMargin, unrealizedPct, estimatedLiqPrice };
}

/** 计算账户汇总指标 */
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

/** 计算累计盈亏百分比 */
export function computeTotalPnlPct(bot: AutoBot, totalPnl: number): number {
  return bot.initial_capital > 0 ? (totalPnl / bot.initial_capital) * 100 : 0;
}

/** 计算距强平百分比 */
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

/** 体积缩写格式化 */
export function formatVolume(v: number): string {
  if (v >= 1e9) return (v / 1e9).toFixed(2) + 'B';
  if (v >= 1e6) return (v / 1e6).toFixed(2) + 'M';
  if (v >= 1e3) return (v / 1e3).toFixed(2) + 'K';
  return v.toFixed(2);
}

/** 紧凑数字格式化（千分位，最多 2 位小数） */
export function formatCompact(n: number, decimals = 2): string {
  return n.toLocaleString('en-US', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

/** 带符号格式化：>0 加 +，<0 保留 -，===0 不加符号 */
export function formatSigned(n: number, decimals = 2): string {
  if (n > 0) return '+' + formatCompact(n, decimals);
  if (n < 0) return formatCompact(n, decimals);
  return formatCompact(n, decimals);
}

/** 相对时间格式化："2m前"、"3h前"、"2d前" */
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

/** 运行时长格式化："3d"、"6h"、"42m" */
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
