import { memo } from 'react';
import type { MarketLineProps } from './types';
import { FlashPrice } from '../../../components/FlashPrice';

/** 决策间隔秒数 → 可读格式 */
function formatInterval(secs: number): string {
  if (secs <= 0) return '';
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h`;
}

/**
 * 市场行原子组件。
 * 上行：symbol + 交易所 ｜ 右侧：杠杆 · 决策间隔 · 状态点
 * 下行：最新价（闪烁）+ 涨跌幅
 *
 * 24H 高/低/量由底部 StickyMarket 展示，此处不重复。
 */
export const MarketLine = memo(
  ({
    symbol,
    exchange,
    latestPrice,
    changePct,
    leverage,
    decideIntervalSecs,
    status,
  }: MarketLineProps) => {
    const chgClass =
      changePct > 0
        ? 'text-success-text'
        : changePct < 0
          ? 'text-danger-text'
          : 'text-on-surface-tertiary';
    const arrow = changePct > 0 ? '▲' : changePct < 0 ? '▼' : '';
    const isRunning = status === 'running';
    const statusDotClass = isRunning ? 'bg-success' : 'bg-on-surface-muted';

    return (
      <div className="flex flex-col gap-1">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-baseline gap-1.5">
            <span className="text-sm font-bold">{symbol}</span>
            <span className="text-2xs text-on-surface-tertiary">· {exchange}</span>
          </div>
          {/* bot 配置微标签 */}
          <div className="flex items-center gap-1.5 text-2xs text-on-surface-tertiary">
            <span className="font-mono tabular-nums font-semibold text-on-surface-secondary">
              {leverage}x
            </span>
            {decideIntervalSecs > 0 && (
              <>
                <span className="text-on-surface-faint">·</span>
                <span className="font-mono tabular-nums">{formatInterval(decideIntervalSecs)}</span>
              </>
            )}
            <span className="text-on-surface-faint">·</span>
            <span
              className={`inline-block w-1.5 h-1.5 rounded-full ${statusDotClass} ${isRunning ? 'animate-pulse' : ''}`}
            />
          </div>
        </div>
        <div className="flex items-baseline justify-between gap-2">
          <FlashPrice
            price={latestPrice}
            className="text-2xl font-bold leading-none tracking-tight"
          />
          <span className={`text-sm font-semibold font-mono tabular-nums ${chgClass}`}>
            {arrow} {changePct > 0 ? '+' : ''}
            {changePct.toFixed(2)}%
          </span>
        </div>
      </div>
    );
  },
);
