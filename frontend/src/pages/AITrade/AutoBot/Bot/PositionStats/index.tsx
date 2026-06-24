import { memo, useMemo } from 'react'
import type { AutoBot } from '@/service/types'
import { usePositionContext } from '../../../context/PositionContext'

interface Props {
  bot: AutoBot
  latestPrice: number
}

// ── 字体规范（全页面统一） ──────────────────────────────
// Hero value:   text-xl font-mono font-semibold tabular-nums (20px)
// Primary value: text-sm font-mono tabular-nums (14px)
// Label:        text-[11px] uppercase tracking-wider text-on-surface-tertiary
// Sub text:     text-[10px] text-on-surface-muted
// ────────────────────────────────────────────────────────

const pnlColor = (v: number) =>
  v > 0 ? 'text-emerald-400' : v < 0 ? 'text-red-400' : 'text-on-surface'

const PositionStats = ({ bot, latestPrice }: Props) => {
  const { position } = usePositionContext()

  const hasPosition = useMemo(
    () => !!position && position.status === 'open' && position.size > 0,
    [position],
  )

  // ── 前端实时计算（不依赖 position WS 的 pnl 字段） ──
  const unrealizedPnl = useMemo(() => {
    if (!hasPosition || latestPrice <= 0) return 0
    const dir = position!.side === 'long' ? 1 : -1
    return (latestPrice - position!.entryPrice) * position!.size * dir
  }, [hasPosition, position, latestPrice])

  const usedMargin = hasPosition ? position!.margin : 0
  const accountBalance = bot.initial_capital + bot.total_pnl + unrealizedPnl
  const freeMargin = accountBalance - usedMargin
  const unrealizedPct = useMemo(() => {
    if (!hasPosition || position!.margin <= 0) return 0
    return (unrealizedPnl / position!.margin) * 100
  }, [hasPosition, position, unrealizedPnl])

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
              {hasPosition ? `${unrealizedPnl >= 0 ? '+' : ''}${unrealizedPnl.toFixed(2)}` : '-'}
            </div>
          </div>
        </div>
      </div>

      {/* ── 第二区：仓位 + 行情 ── */}
      <div className="px-4 py-2 border-t border-line-subtle/50 grid grid-cols-3 sm:grid-cols-6 gap-x-4 gap-y-1.5">
        <Stat label="最新价">
          <span className="text-on-surface">{latestPrice > 0 ? latestPrice.toFixed(2) : '-'}</span>
        </Stat>
        <Stat label="持仓方向">
          {hasPosition ? (
            <span className={position!.side === 'long' ? 'text-emerald-400' : 'text-red-400'}>
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
            <span className="text-red-400">{position!.liquidationPrice.toFixed(2)}</span>
          ) : (
            <span className="text-on-surface-muted">-</span>
          )}
        </Stat>
      </div>

      {/* ── 第三区：Bot 配置 ── */}
      <div className="px-4 py-2 border-t border-line-subtle/50 grid grid-cols-3 sm:grid-cols-6 gap-x-4 gap-y-1.5">
        <Stat label="杠杆">
          <span className="text-on-surface">{bot.leverage}x</span>
        </Stat>
        <Stat label="止损">
          <span className="text-red-400">{bot.stop_loss > 0 ? bot.stop_loss.toFixed(2) : '-'}</span>
        </Stat>
        <Stat label="止盈">
          <span className="text-emerald-400">
            {bot.take_profit > 0 ? bot.take_profit.toFixed(2) : '-'}
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
  )
}

/** 统一字段组件：label + value */
const Stat = ({ label, children }: { label: string; children: React.ReactNode }) => (
  <div className="min-w-0">
    <div className="text-[11px] uppercase tracking-wider text-on-surface-tertiary mb-0.5">
      {label}
    </div>
    <div className="text-sm font-mono tabular-nums truncate">{children}</div>
  </div>
)

export default memo(PositionStats)
