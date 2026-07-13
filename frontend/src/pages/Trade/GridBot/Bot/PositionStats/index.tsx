import { memo, useMemo } from 'react';
import type { GridBot } from '@/service/types';
import Stat from '@/components/Stat';
import { pnlColor } from '../../../components/utils/utils';
import { useBot } from '../../../context/BotContext';

interface Props {
  bot: GridBot;
  latestPrice: number;
}

// ── 字体规范（全页面统一） ──────────────────────────────
// Hero value:   text-xl font-mono font-semibold tabular-nums (20px)
// Primary value: text-sm font-mono tabular-nums (14px)
// Label:        text-[11px] uppercase tracking-wider text-on-surface-tertiary
// Sub text:     text-[10px] text-on-surface-muted
// ────────────────────────────────────────────────────────

const PositionStats = ({ bot, latestPrice }: Props) => {
  const { gridLevels } = useBot();
  const b = bot;
  const filledCount = Math.min(b.grid_filled_count, b.grid_count);

  // ── 前端实时计算未实现盈亏（基于 grid levels + 最新价） ──
  const { unrealizedPnl, usedMargin } = useMemo(() => {
    if (!gridLevels || gridLevels.length === 0 || latestPrice <= 0) {
      return { unrealizedPnl: 0, usedMargin: 0 };
    }
    let pnl = 0;
    let margin = 0;
    for (const level of gridLevels) {
      const qty = Math.abs(level.hold_quantity);
      if (qty <= 0) continue;
      // 持仓方向：buy=多，sell=空
      const dir = level.side === 'buy' ? 1 : -1;
      const avgPrice = level.avg_buy_price > 0 ? level.avg_buy_price : level.buy_price;
      pnl += (latestPrice - avgPrice) * qty * dir;
      // 保证金 = 持仓价值 / 杠杆
      margin += (qty * avgPrice) / b.leverage;
    }
    return { unrealizedPnl: pnl, usedMargin: margin };
  }, [gridLevels, latestPrice, b.leverage]);

  const accountBalance = b.initial_capital + b.total_pnl + unrealizedPnl;
  const freeMargin = accountBalance - usedMargin;
  const totalPnl = b.total_pnl + unrealizedPnl;

  return (
    <div className="border-b border-line-subtle">
      {/* ── 第一区：账户概览（Hero） ── */}
      <div className="px-4 py-3 flex items-center gap-6">
        {/* 账户余额 - Hero number */}
        <div className="shrink-0">
          <div className="text-[11px] uppercase tracking-wider text-on-surface-tertiary mb-0.5">
            账户余额
          </div>
          <div className="text-xl font-mono font-semibold tabular-nums text-on-surface">
            {accountBalance.toFixed(2)}
          </div>
        </div>

        {/* 分隔线 */}
        <div className="h-10 w-px bg-line-subtle shrink-0" />

        {/* 保证金三列 */}
        <div className="flex items-center gap-5 flex-1 min-w-0">
          <div>
            <div className="text-[11px] uppercase tracking-wider text-on-surface-tertiary mb-0.5">
              已用保证金
            </div>
            <div className="text-sm font-mono tabular-nums text-on-surface">
              {usedMargin > 0 ? usedMargin.toFixed(2) : '-'}
            </div>
          </div>
          <div>
            <div className="text-[11px] uppercase tracking-wider text-on-surface-tertiary mb-0.5">
              剩余保证金
            </div>
            <div className="text-sm font-mono tabular-nums text-on-surface">
              {freeMargin.toFixed(2)}
            </div>
          </div>
          <div>
            <div className="text-[11px] uppercase tracking-wider text-on-surface-tertiary mb-0.5">
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

      {/* ── 第二区：网格 + 行情 ── */}
      <div className="px-4 py-2 border-t border-line-subtle/50 grid grid-cols-3 sm:grid-cols-6 gap-x-4 gap-y-1.5">
        <Stat label="最新价">
          <span className="text-on-surface">{latestPrice > 0 ? latestPrice.toFixed(2) : '-'}</span>
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

      {/* ── 第三区：Bot 配置 ── */}
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
};

export default memo(PositionStats);
