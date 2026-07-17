import { memo, useMemo } from 'react';
import type { AutoBot } from '@/service/types';
import { Stat } from '@/components/Stat';
import { usePositionContext } from '../../../context/PositionContext';
import { FlashPrice } from '../../../components/FlashPrice';
import { pnlColor } from '../../../components/utils/utils';

interface Props {
  bot: AutoBot;
  latestPrice: number;
}

export const PositionStats = memo(({ bot, latestPrice }: Props) => {
  const { position } = usePositionContext();

  const hasPosition = useMemo(
    () => !!position && position.status === 'open' && position.size > 0,
    [position],
  );

  const unrealizedPnl = useMemo(() => {
    if (!hasPosition || latestPrice <= 0) return 0;
    const dir = position!.side === 'long' ? 1 : -1;
    return (latestPrice - position!.entryPrice) * position!.size * dir;
  }, [hasPosition, position, latestPrice]);

  const usedMargin = hasPosition ? position!.margin : 0;
  const accountBalance = bot.initial_capital + bot.total_pnl + unrealizedPnl;
  const freeMargin = accountBalance - usedMargin;
  const unrealizedPct = useMemo(() => {
    if (!hasPosition || position!.margin <= 0) return 0;
    return (unrealizedPnl / position!.margin) * 100;
  }, [hasPosition, position, unrealizedPnl]);

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
              {hasPosition ? `${unrealizedPnl >= 0 ? '+' : ''}${unrealizedPnl.toFixed(2)}` : '-'}
            </div>
          </div>
        </div>
      </div>

      {}
      <div className="px-4 py-2 border-t border-line-subtle/50 grid grid-cols-3 sm:grid-cols-6 gap-x-4 gap-y-1.5">
        <Stat label="最新价">
          <FlashPrice price={latestPrice} className="text-on-surface" />
        </Stat>
        <Stat label="持仓方向">
          {hasPosition ? (
            <span className={position!.side === 'long' ? 'text-success-text' : 'text-danger-text'}>
              {position!.side === 'long' ? '多' : '空'}
            </span>
          ) : (
            <span className="text-on-surface-muted">空仓</span>
          )}
        </Stat>
        <Stat label="持仓量">
          {hasPosition ? (
            <span className="text-on-surface">{position!.size.toFixed(4)}</span>
          ) : (
            <span className="text-on-surface-muted">-</span>
          )}
        </Stat>
        <Stat label="开仓价">
          {hasPosition ? (
            <span className="text-on-surface">{position!.entryPrice.toFixed(2)}</span>
          ) : (
            <span className="text-on-surface-muted">-</span>
          )}
        </Stat>
        <Stat label="收益率">
          {hasPosition ? (
            <span className={pnlColor(unrealizedPnl)}>
              {unrealizedPct >= 0 ? '+' : ''}
              {unrealizedPct.toFixed(2)}%
            </span>
          ) : (
            <span className="text-on-surface-muted">-</span>
          )}
        </Stat>
        <Stat label="强平价">
          {hasPosition && position!.liquidationPrice != null ? (
            <span className="text-danger-text">{position!.liquidationPrice.toFixed(2)}</span>
          ) : (
            <span className="text-on-surface-muted">-</span>
          )}
        </Stat>
      </div>

      {}
      <div className="px-4 py-2 border-t border-line-subtle/50 grid grid-cols-3 sm:grid-cols-6 gap-x-4 gap-y-1.5">
        <Stat label="杠杆">
          <span className="text-on-surface">{bot.leverage}x</span>
        </Stat>
        <Stat label="止损">
          <span className="text-danger-text">
            {hasPosition && position!.stopLoss != null && position!.stopLoss > 0
              ? position!.stopLoss.toFixed(2)
              : '-'}
          </span>
        </Stat>
        <Stat label="止盈">
          <span className="text-success-text">
            {hasPosition && position!.takeProfit != null && position!.takeProfit > 0
              ? position!.takeProfit.toFixed(2)
              : '-'}
          </span>
        </Stat>
        <Stat label="仓位上限">
          <span className="text-on-surface">{bot.max_position_pct}%</span>
        </Stat>
        <Stat label="决策间隔">
          <span className="text-on-surface">{bot.decide_interval_secs}s</span>
        </Stat>
        <Stat label="市况">
          <span className="text-on-surface">{bot.market_regime || '-'}</span>
        </Stat>
      </div>
    </div>
  );
});
