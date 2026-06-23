import { memo, useMemo } from 'react'
import type { AutoBot } from '@/service/types'
import { formatPnl } from '../../../components/utils/utils'
import { usePositionContext } from '../../../context/PositionContext'

interface Props {
  bot: AutoBot
  latestPrice: number
}

interface FieldProps {
  label: string
  children: React.ReactNode
  className?: string
}

/** 单个字段：label + value 紧凑排列 */
const Field = ({ label, children, className = '' }: FieldProps) => (
  <div className={`flex items-baseline gap-1.5 min-w-0 ${className}`}>
    <span className="text-on-surface-tertiary text-[10px] uppercase tracking-wide shrink-0">
      {label}
    </span>
    <span className="font-mono text-xs truncate">{children}</span>
  </div>
)

const PositionStats = ({ bot, latestPrice }: Props) => {
  const { position } = usePositionContext()

  const hasPosition = useMemo(
    () => !!position && position.status === 'open' && position.size > 0,
    [position],
  )

  const unrealizedPnl = hasPosition ? position!.unrealizedPnl : 0
  const pnlColor =
    unrealizedPnl > 0 ? 'text-emerald-400' : unrealizedPnl < 0 ? 'text-red-400' : 'text-on-surface'

  return (
    <div className="px-4 py-2.5 border-b border-line-subtle space-y-2">
      {/* 第一区：实时仓位信息（大屏一行，小屏两行网格） */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-x-4 gap-y-1.5">
        <Field label="最新">
          <span className="text-on-surface font-medium">
            {latestPrice > 0 ? latestPrice.toFixed(2) : '-'}
          </span>
        </Field>

        <Field label="仓位">
          {hasPosition ? (
            <span className={position!.side === 'long' ? 'text-emerald-400' : 'text-red-400'}>
              {position!.side === 'long' ? '多' : '空'} {position!.size.toFixed(4)}
            </span>
          ) : (
            <span className="text-on-surface-tertiary">空仓</span>
          )}
        </Field>

        <Field label="开仓价">
          {hasPosition ? (
            <span className="text-on-surface">{position!.entryPrice.toFixed(2)}</span>
          ) : (
            <span className="text-on-surface-tertiary">-</span>
          )}
        </Field>

        <Field label="未实现">
          {hasPosition ? (
            <span className={pnlColor}>{formatPnl(unrealizedPnl)}</span>
          ) : (
            <span className="text-on-surface-tertiary">-</span>
          )}
        </Field>

        <Field label="强平">
          {hasPosition && position!.liquidationPrice != null ? (
            <span className="text-red-400">{position!.liquidationPrice.toFixed(2)}</span>
          ) : (
            <span className="text-on-surface-tertiary">-</span>
          )}
        </Field>

        <Field label="保证金">
          {hasPosition ? (
            <span className="text-on-surface">{position!.margin.toFixed(2)}</span>
          ) : (
            <span className="text-on-surface-tertiary">-</span>
          )}
        </Field>
      </div>

      {/* 第二区：bot 配置参数（大屏一行，小屏两行网格） */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-x-4 gap-y-1.5 pt-1.5 border-t border-line-subtle/50">
        <Field label="杠杆">
          <span className="text-on-surface">{bot.leverage}x</span>
        </Field>
        <Field label="止损">
          <span className="text-red-400">{bot.stop_loss > 0 ? bot.stop_loss.toFixed(2) : '-'}</span>
        </Field>
        <Field label="止盈">
          <span className="text-emerald-400">
            {bot.take_profit > 0 ? bot.take_profit.toFixed(2) : '-'}
          </span>
        </Field>
        <Field label="仓位上限">
          <span className="text-on-surface">{bot.max_position_pct}%</span>
        </Field>
        <Field label="决策间隔">
          <span className="text-on-surface">{bot.decide_interval_secs}s</span>
        </Field>
        <Field label="市况">
          <span className="text-on-surface">{bot.market_regime || '-'}</span>
        </Field>
      </div>
    </div>
  )
}

export default memo(PositionStats)
