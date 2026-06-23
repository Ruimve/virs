import { memo } from 'react'
import type { GridBot } from '@/service/types'
import { formatPnlShort } from '../../../components/utils/utils'

interface Props {
  bot: GridBot
  latestPrice: number
}

interface FieldProps {
  label: string
  children: React.ReactNode
  className?: string
}

/** 单个字段：label + value 紧凑排列（对齐 PositionStats） */
const Field = ({ label, children, className = '' }: FieldProps) => (
  <div className={`flex items-baseline gap-1.5 min-w-0 ${className}`}>
    <span className="text-on-surface-tertiary text-[10px] uppercase tracking-wide shrink-0">
      {label}
    </span>
    <span className="font-mono text-xs truncate">{children}</span>
  </div>
)

const PositionStats = ({ bot, latestPrice }: Props) => {
  const b = bot
  const filledCount = Math.min(b.grid_filled_count, b.grid_count)

  return (
    <div className="px-4 py-2.5 border-b border-line-subtle space-y-2">
      {/* 第一区：实时行情 + 网格状态（大屏一行，小屏两行网格） */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-x-4 gap-y-1.5">
        <Field label="最新">
          <span className="text-on-surface font-medium">
            {latestPrice > 0 ? latestPrice.toFixed(2) : '-'}
          </span>
        </Field>

        <Field label="已实现">
          <span className={b.total_pnl >= 0 ? 'text-emerald-400' : 'text-red-400'}>
            {formatPnlShort(b.total_pnl)}
          </span>
        </Field>

        <Field label="未实现">
          <span className={b.unrealized_pnl >= 0 ? 'text-emerald-400' : 'text-red-400'}>
            {formatPnlShort(b.unrealized_pnl)}
          </span>
        </Field>

        <Field label="总交易">
          <span className="text-on-surface">{b.total_trades} 笔</span>
        </Field>

        <Field label="网格填充">
          <span className="text-on-surface">
            {filledCount}/{b.grid_count}
          </span>
        </Field>

        <Field label="利润率">
          <span className={b.grid_profit_pct >= 0 ? 'text-emerald-400' : 'text-red-400'}>
            {b.grid_profit_pct}%
          </span>
        </Field>
      </div>

      {/* 第二区：bot 配置参数（大屏一行，小屏两行网格） */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-x-4 gap-y-1.5 pt-1.5 border-t border-line-subtle/50">
        <Field label="杠杆">
          <span className="text-on-surface">{b.leverage}x</span>
        </Field>
        <Field label="网格数">
          <span className="text-on-surface">{b.grid_count}</span>
        </Field>
        {b.upper_price > 0 ? (
          <Field label="区间">
            <span className="text-on-surface">
              {b.lower_price.toFixed(0)}—{b.upper_price.toFixed(0)}
            </span>
          </Field>
        ) : (
          <Field label="区间">
            <span className="text-on-surface-tertiary">-</span>
          </Field>
        )}
        <Field label="每格量">
          <span className="text-on-surface">{b.quantity_per_grid}</span>
        </Field>
        <Field label="动态调整">
          <span className={b.dynamic_adjust ? 'text-emerald-400' : 'text-on-surface-tertiary'}>
            {b.dynamic_adjust ? '开启' : '关闭'}
          </span>
        </Field>
        <Field label="市况">
          <span className="text-on-surface">{b.market_regime || '-'}</span>
        </Field>
      </div>
    </div>
  )
}

export default memo(PositionStats)
