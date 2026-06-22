import { memo, useMemo } from 'react'
import type { AutoBot, AutoTrade } from '@/service/types'
import { formatPnl } from '../../components/utils/utils'

interface Props {
  bot: AutoBot
  latestPrice: number
  trades: AutoTrade[]
}

const PositionStats = ({ bot, latestPrice, trades }: Props) => {
  const hasPosition = useMemo(
    () => bot.current_side && bot.current_side !== 'none',
    [bot.current_side],
  )
  const totalFee = useMemo(() => trades.reduce((sum, t) => sum + (t.fee || 0), 0), [trades])

  // 根据最新价实时计算未实现 PnL（不依赖后端持久化）
  const unrealizedPnl = useMemo(
    () =>
      hasPosition && latestPrice > 0 && bot.entry_price > 0
        ? bot.current_side === 'long'
          ? (latestPrice - bot.entry_price) * bot.position_size
          : (bot.entry_price - latestPrice) * bot.position_size
        : 0,
    [hasPosition, latestPrice, bot.entry_price, bot.current_side, bot.position_size],
  )

  return (
    <div className="px-4 py-2.5 border-b border-line-subtle">
      {/* 第一行：最新价 + 持仓 + PnL + 手续费 */}
      <div className="flex items-center gap-4 flex-wrap text-xs">
        {/* 最新价 */}
        <div className="flex items-center gap-1.5">
          <span className="text-on-surface-tertiary">最新</span>
          <span className="text-on-surface font-mono font-medium">
            {latestPrice > 0 ? latestPrice.toFixed(2) : '-'}
          </span>
        </div>

        <span className="w-px h-3 bg-line-subtle" />

        {/* 持仓 */}
        {hasPosition ? (
          <div className="flex items-center gap-1.5">
            <span
              className={`font-medium ${bot.current_side === 'long' ? 'text-emerald-400' : 'text-red-400'}`}
            >
              {bot.current_side === 'long' ? '做多' : '做空'}
            </span>
            <span className="text-on-surface-tertiary">
              <span className="text-on-surface font-mono">{bot.position_size.toFixed(4)}</span> @{' '}
              <span className="text-on-surface font-mono">{bot.entry_price.toFixed(2)}</span>
            </span>
          </div>
        ) : (
          <span className="text-on-surface-tertiary">空仓</span>
        )}

        <span className="w-px h-3 bg-line-subtle" />

        {/* 未实现 PnL */}
        {hasPosition && (
          <>
            <div className="flex items-center gap-1">
              <span className="text-on-surface-tertiary">未实现</span>
              {formatPnl(unrealizedPnl)}
            </div>
            <span className="w-px h-3 bg-line-subtle" />
          </>
        )}

        {/* 累计 PnL */}
        <div className="flex items-center gap-1">
          <span className="text-on-surface-tertiary">累计</span>
          {formatPnl(bot.total_pnl)}
        </div>

        <span className="w-px h-3 bg-line-subtle" />

        {/* 累计手续费 */}
        <div className="flex items-center gap-1">
          <span className="text-on-surface-tertiary">手续费</span>
          <span className="text-amber-400 font-mono">-{totalFee.toFixed(4)}</span>
        </div>
      </div>

      {/* 第二行：配置参数 */}
      <div className="flex items-center gap-4 flex-wrap text-[11px] text-on-surface-tertiary mt-1.5">
        <span>
          杠杆 <span className="text-on-surface font-mono">{bot.leverage}x</span>
        </span>
        <span>
          止损{' '}
          <span className="text-red-400 font-mono">
            {bot.stop_loss > 0 ? bot.stop_loss.toFixed(2) : '-'}
          </span>
        </span>
        <span>
          止盈{' '}
          <span className="text-emerald-400 font-mono">
            {bot.take_profit > 0 ? bot.take_profit.toFixed(2) : '-'}
          </span>
        </span>
        <span>
          仓位上限 <span className="text-on-surface font-mono">{bot.max_position_pct}%</span>
        </span>
        <span>
          决策间隔 <span className="text-on-surface font-mono">{bot.decide_interval_secs}s</span>
        </span>
        {bot.market_regime && (
          <span>
            市况 <span className="text-on-surface font-mono">{bot.market_regime}</span>
          </span>
        )}
      </div>
    </div>
  )
}

export default memo(PositionStats)
