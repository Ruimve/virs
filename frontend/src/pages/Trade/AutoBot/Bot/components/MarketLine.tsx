import { memo } from 'react';
import type { MarketLineProps } from './types';
import { FlashPrice } from '../../../components/FlashPrice';

/**
 * 市场行原子组件。
 *
 * 布局：
 *   上行：symbol + 杠杆 + 交易所（交易标的标识）
 *   下行：最新价（闪烁）+ 涨跌幅
 *
 * 设计原则：
 *   - symbol 为页面主标识，使用 text-on-surface（surface-1 背景专用文本色）
 *   - 杠杆为关键交易参数，交易所为上下文标签
 *   - 涨跌幅仅用符号(+/-) + 颜色区分方向，不加箭头避免冗余
 *   - 运行状态由 Header 展示，决策间隔由 StrategyBlock 展示，24H 由 StickyMarket 展示
 */
export const MarketLine = memo(
  ({ symbol, exchange, latestPrice, changePct, leverage }: MarketLineProps) => {
    const chgClass =
      changePct > 0
        ? 'text-success-text'
        : changePct < 0
          ? 'text-danger-text'
          : 'text-on-surface-tertiary';

    return (
      <div className="flex flex-col gap-1">
        {/* 标识行：symbol + 杠杆 + 交易所 */}
        <div className="flex items-baseline gap-1.5">
          <span className="text-base font-bold leading-none tracking-tight text-on-surface">
            {symbol}
          </span>
          <span className="text-on-surface-faint text-2xs">·</span>
          <span className="text-2xs font-mono font-semibold tabular-nums text-on-surface-secondary">
            {leverage}x
          </span>
          <span className="text-on-surface-faint text-2xs">·</span>
          <span className="text-2xs uppercase tracking-wide text-on-surface-tertiary">
            {exchange}
          </span>
        </div>
        {/* 价格行 */}
        <div className="flex items-baseline justify-between gap-2">
          <FlashPrice
            price={latestPrice}
            className="text-2xl font-bold leading-none tracking-tight text-on-surface"
          />
          <span className={`text-sm font-semibold font-mono tabular-nums ${chgClass}`}>
            {changePct > 0 ? '+' : ''}
            {changePct.toFixed(2)}%
          </span>
        </div>
      </div>
    );
  },
);
