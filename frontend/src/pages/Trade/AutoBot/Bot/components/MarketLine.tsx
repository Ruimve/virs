import { memo } from 'react';
import type { MarketLineProps } from './types';
import { FlashPrice } from '../../../components/FlashPrice';

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
        {}
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
        {}
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
