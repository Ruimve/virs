import { memo, useMemo } from 'react';
import type { GridBot } from '@/service/types';
import { Stat } from '@/components/Stat';
import { FlashPrice } from '../../../components/FlashPrice';
import { pnlColor } from '../../../components/utils/utils';
import { useBot } from '../../../context/BotContext';

interface Props {
  bot: GridBot;
  latestPrice: number;
}

export const PositionStats = memo(({ bot, latestPrice }: Props) => {
  const { gridLevels } = useBot();
  const b = bot;
  const filledCount = Math.min(b.grid_filled_count, b.grid_count);

  const { unrealizedPnl, usedMargin } = useMemo(() => {
    if (!gridLevels || gridLevels.length === 0 || latestPrice <= 0) {
      return { unrealizedPnl: 0, usedMargin: 0 };
    }
    let pnl = 0;
    let margin = 0;
    for (const level of gridLevels) {
      const qty = Math.abs(level.hold_quantity);
      if (qty <= 0) continue;

      const dir = level.side === 'buy' ? 1 : -1;
      const avgPrice = level.avg_buy_price > 0 ? level.avg_buy_price : level.buy_price;
      pnl += (latestPrice - avgPrice) * qty * dir;

      margin += (qty * avgPrice) / b.leverage;
    }
    return { unrealizedPnl: pnl, usedMargin: margin };
  }, [gridLevels, latestPrice, b.leverage]);

  const accountBalance = b.initial_capital + b.total_pnl + unrealizedPnl;
  const freeMargin = accountBalance - usedMargin;
  const totalPnl = b.total_pnl + unrealizedPnl;

  return (
    <div className="border-b border-line-subtle">
      {}
      <div className="px-4 py-3 flex items-center gap-6">
        {}
        <div className="shrink-0">
          <div className="text-caption uppercase tracking-wider text-on-surface-tertiary mb-0.5">
            账户余额
          </div>
          <div className="text-xl font-mono font-semibold tabular-nums text-on-surface">
            {accountBalance.toFixed(2)}
          </div>
        </div>

        {}
        <div className="h-10 w-px bg-line-subtle shrink-0" />

        {}
        <div className="flex items-center gap-5 flex-1 min-w-0">
          <div>
            <div className="text-caption uppercase tracking-wider text-on-surface-tertiary mb-0.5">
              已用保证金
            </div>
            <div className="text-sm font-mono tabular-nums text-on-surface">
              {usedMargin > 0 ? usedMargin.toFixed(2) : '-'}
            </div>
          </div>
          <div>
            <div className="text-caption uppercase tracking-wider text-on-surface-tertiary mb-0.5">
              剩余保证金
            </div>
            <div className="text-sm font-mono tabular-nums text-on-surface">
              {freeMargin.toFixed(2)}
            </div>
          </div>
          <div>
            <div className="text-caption uppercase tracking-wider text-on-surface-tertiary mb-0.5">
              未实现盈亏
            </div>
            <div className={`text-sm font-mono tabular-nums ${pnlColor(unrealizedPnl)}`}>
              {unrealizedPnl !== 0
                ? `${unrealizedPnl >= 0 ? '+' : ''}${unrealizedPnl.toFixed(2)}`
                : '-'}
            </div>
          </div>
        </div>
      </div>

      {}
      <div className="px-4 py-2 border-t border-line-subtle/50 grid grid-cols-3 sm:grid-cols-6 gap-x-4 gap-y-1.5">
        <Stat label="最新价">
          <FlashPrice price={latestPrice} className="text-on-surface" />
        </Stat>
        <Stat label="累计盈亏">
          <span className={pnlColor(totalPnl)}>
            {totalPnl >= 0 ? '+' : ''}
            {totalPnl.toFixed(2)}
          </span>
        </Stat>
        <Stat label="已实现">
          <span className={pnlColor(b.total_pnl)}>
            {b.total_pnl >= 0 ? '+' : ''}
            {b.total_pnl.toFixed(2)}
          </span>
        </Stat>
        <Stat label="总交易">
          <span className="text-on-surface">{b.total_trades} 笔</span>
        </Stat>
        <Stat label="网格填充">
          <span className="text-on-surface">
            {filledCount}/{b.grid_count}
          </span>
        </Stat>
        <Stat label="利润率">
          <span className={b.grid_profit_pct >= 0 ? 'text-success-text' : 'text-danger-text'}>
            {b.grid_profit_pct}%
          </span>
        </Stat>
      </div>

      {}
      <div className="px-4 py-2 border-t border-line-subtle/50 grid grid-cols-3 sm:grid-cols-6 gap-x-4 gap-y-1.5">
        <Stat label="杠杆">
          <span className="text-on-surface">{b.leverage}x</span>
        </Stat>
        <Stat label="网格数">
          <span className="text-on-surface">{b.grid_count}</span>
        </Stat>
        {b.upper_price > 0 ? (
          <Stat label="区间">
            <span className="text-on-surface">
              {b.lower_price.toFixed(0)}—{b.upper_price.toFixed(0)}
            </span>
          </Stat>
        ) : (
          <Stat label="区间">
            <span className="text-on-surface-muted">-</span>
          </Stat>
        )}
        <Stat label="每格量">
          <span className="text-on-surface">{b.quantity_per_grid}</span>
        </Stat>
        <Stat label="动态调整">
          <span className={b.dynamic_adjust ? 'text-success-text' : 'text-on-surface-muted'}>
            {b.dynamic_adjust ? '开启' : '关闭'}
          </span>
        </Stat>
        <Stat label="市况">
          <span className="text-on-surface">{b.market_regime || '-'}</span>
        </Stat>
      </div>
    </div>
  );
});
