import { memo, useMemo, type ReactNode } from 'react';
import type { AutoBot } from '@/service/types';
import type { PositionWsEvent } from '@/service/ws';
import { Stat } from '@/components/Stat';
import { usePositionContext, type PositionSide } from '../../../context/PositionContext';
import { FlashPrice } from '../../../components/FlashPrice';
import { pnlColor } from '../../../components/utils/utils';

interface Props {
  bot: AutoBot;
  latestPrice: number;
}

interface SideMetrics {
  hasPosition: boolean;
  unrealizedPnl: number;
  usedMargin: number;
  unrealizedPct: number;
}

const EMPTY_METRICS: SideMetrics = {
  hasPosition: false,
  unrealizedPnl: 0,
  usedMargin: 0,
  unrealizedPct: 0,
};

function computeMetrics(
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
  return { hasPosition: true, unrealizedPnl, usedMargin, unrealizedPct };
}

const SIDE_LABEL: Record<PositionSide, string> = { long: '多', short: '空' };

function SideRow({
  side,
  position,
  metrics,
}: {
  side: PositionSide;
  position: PositionWsEvent | null;
  metrics: SideMetrics;
}) {
  const isLong = side === 'long';
  const sideColor = isLong ? 'text-success-text' : 'text-danger-text';
  return (
    <div className="px-4 py-2 border-t border-line-subtle/50 grid grid-cols-3 sm:grid-cols-7 gap-x-4 gap-y-1.5">
      <Stat label={isLong ? '多头方向' : '空头方向'}>
        <span className={sideColor}>{SIDE_LABEL[side]}</span>
      </Stat>
      <Stat label="持仓量">
        {metrics.hasPosition ? (
          <span className="text-on-surface">{position!.quantity.toFixed(4)}</span>
        ) : (
          <span className="text-on-surface-muted">-</span>
        )}
      </Stat>
      <Stat label="开仓价">
        {metrics.hasPosition ? (
          <span className="text-on-surface">{position!.entryPrice.toFixed(2)}</span>
        ) : (
          <span className="text-on-surface-muted">-</span>
        )}
      </Stat>
      <Stat label="止损">
        <span className="text-danger-text">
          {metrics.hasPosition && position!.stopLoss != null && position!.stopLoss > 0
            ? position!.stopLoss.toFixed(2)
            : '-'}
        </span>
      </Stat>
      <Stat label="止盈">
        <span className="text-success-text">
          {metrics.hasPosition && position!.takeProfit != null && position!.takeProfit > 0
            ? position!.takeProfit.toFixed(2)
            : '-'}
        </span>
      </Stat>
      <Stat label="收益率">
        {metrics.hasPosition ? (
          <span className={pnlColor(metrics.unrealizedPnl)}>
            {metrics.unrealizedPct >= 0 ? '+' : ''}
            {metrics.unrealizedPct.toFixed(2)}%
          </span>
        ) : (
          <span className="text-on-surface-muted">-</span>
        )}
      </Stat>
      <Stat label="未实现盈亏">
        {metrics.hasPosition ? (
          <span className={pnlColor(metrics.unrealizedPnl)}>
            {metrics.unrealizedPnl >= 0 ? '+' : ''}
            {metrics.unrealizedPnl.toFixed(2)}
          </span>
        ) : (
          <span className="text-on-surface-muted">-</span>
        )}
      </Stat>
    </div>
  );
}

const EmptyRow = ({ children }: { children: ReactNode }) => (
  <div className="px-4 py-2 border-t border-line-subtle/50 text-sm text-on-surface-muted">
    {children}
  </div>
);

export const PositionStats = memo(({ bot, latestPrice }: Props) => {
  const { positions } = usePositionContext();

  const longMetrics = useMemo(
    () => computeMetrics(positions.long, latestPrice, bot.leverage),
    [positions.long, latestPrice, bot.leverage],
  );
  const shortMetrics = useMemo(
    () => computeMetrics(positions.short, latestPrice, bot.leverage),
    [positions.short, latestPrice, bot.leverage],
  );

  const totalUnrealizedPnl = longMetrics.unrealizedPnl + shortMetrics.unrealizedPnl;
  const totalUsedMargin = longMetrics.usedMargin + shortMetrics.usedMargin;
  const hasAny = longMetrics.hasPosition || shortMetrics.hasPosition;

  const accountBalance = bot.initial_capital + bot.total_pnl + totalUnrealizedPnl;
  const freeMargin = accountBalance - totalUsedMargin;

  return (
    <div className="border-b border-line-subtle">
      {/* 汇总行 */}
      <div className="px-4 py-3 flex items-center gap-6">
        <div className="shrink-0">
          <div className="text-caption uppercase tracking-wider text-on-surface-tertiary mb-0.5">
            账户余额
          </div>
          <div className="text-xl font-mono font-semibold tabular-nums text-on-surface">
            {accountBalance.toFixed(2)}
          </div>
        </div>

        <div className="h-10 w-px bg-line-subtle shrink-0" />

        <div className="flex items-center gap-5 flex-1 min-w-0">
          <div>
            <div className="text-caption uppercase tracking-wider text-on-surface-tertiary mb-0.5">
              已用保证金
            </div>
            <div className="text-sm font-mono tabular-nums text-on-surface">
              {totalUsedMargin > 0 ? totalUsedMargin.toFixed(2) : '-'}
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
              总未实现盈亏
            </div>
            <div className={`text-sm font-mono tabular-nums ${pnlColor(totalUnrealizedPnl)}`}>
              {hasAny
                ? `${totalUnrealizedPnl >= 0 ? '+' : ''}${totalUnrealizedPnl.toFixed(2)}`
                : '-'}
            </div>
          </div>
        </div>
      </div>

      {/* 多头仓位行 */}
      {longMetrics.hasPosition ? (
        <SideRow side="long" position={positions.long} metrics={longMetrics} />
      ) : null}

      {/* 空头仓位行 */}
      {shortMetrics.hasPosition ? (
        <SideRow side="short" position={positions.short} metrics={shortMetrics} />
      ) : null}

      {/* 空仓状态：两侧均无仓位 */}
      {!hasAny ? <EmptyRow>空仓</EmptyRow> : null}

      {/* 市场数据行 */}
      <div className="px-4 py-2 border-t border-line-subtle/50 grid grid-cols-3 sm:grid-cols-6 gap-x-4 gap-y-1.5">
        <Stat label="最新价">
          <FlashPrice price={latestPrice} className="text-on-surface" />
        </Stat>
        <Stat label="杠杆">
          <span className="text-on-surface">{bot.leverage}x</span>
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
        <Stat label="已实现盈亏">
          <span className={pnlColor(bot.total_pnl)}>
            {bot.total_pnl >= 0 ? '+' : ''}
            {bot.total_pnl.toFixed(2)}
          </span>
        </Stat>
      </div>
    </div>
  );
});
