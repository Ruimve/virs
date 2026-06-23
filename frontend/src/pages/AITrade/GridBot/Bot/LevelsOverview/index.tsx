import { memo } from 'react'
import type { GridLevelInfo } from '@/service'
import { formatSmart } from '../../../components/utils/utils'

interface Props {
  gridLevels: GridLevelInfo[]
}

/** 网格层级概览卡片（右侧栏，精简版） */
const LevelsOverview = ({ gridLevels }: Props) => {
  const holding = gridLevels.filter((l) => Math.abs(l.hold_quantity) > 0)
  const closed = gridLevels.filter(
    (l) => l.buy_filled && l.sell_filled && Math.abs(l.hold_quantity) === 0,
  )
  const waiting = gridLevels.length - holding.length - closed.length

  return (
    <div className="flex flex-col min-h-0">
      <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle shrink-0">
        <span className="text-[10px] text-on-surface-tertiary uppercase tracking-wider">
          网格层级
        </span>
        <span className="text-[10px] text-on-surface-muted">{gridLevels.length}</span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {gridLevels.length === 0 ? (
          <div className="text-center py-6 text-[11px] text-on-surface-tertiary">暂无网格数据</div>
        ) : (
          <>
            {/* 统计摘要 */}
            <div className="grid grid-cols-3 gap-2 px-3 py-2 border-b border-line-subtle text-[10px]">
              <div className="text-center">
                <div className="text-emerald-400 font-mono font-medium">{holding.length}</div>
                <div className="text-on-surface-tertiary">持仓</div>
              </div>
              <div className="text-center">
                <div className="text-on-surface-muted font-mono font-medium">{closed.length}</div>
                <div className="text-on-surface-tertiary">已平</div>
              </div>
              <div className="text-center">
                <div className="text-on-surface-faint font-mono font-medium">{waiting}</div>
                <div className="text-on-surface-tertiary">等待</div>
              </div>
            </div>
            {/* 层级列表（精简） */}
            <div className="divide-y divide-line-subtle">
              {gridLevels.slice(0, 20).map((level) => {
                const isHolding = Math.abs(level.hold_quantity) > 0
                const isClosed =
                  level.buy_filled && level.sell_filled && Math.abs(level.hold_quantity) === 0
                return (
                  <div
                    key={level.level}
                    className="px-3 py-1.5 flex items-center gap-2 text-[11px]"
                  >
                    <span className="text-on-surface-tertiary font-mono w-6 shrink-0">
                      L{level.level}
                    </span>
                    <span
                      className={`inline-block px-1 py-0.5 rounded text-[9px] font-medium shrink-0 ${level.side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}
                    >
                      {level.side === 'buy' ? '多' : '空'}
                    </span>
                    <span className="text-on-surface font-mono flex-1 truncate">
                      {level.buy_price.toFixed(2)} → {level.sell_price.toFixed(2)}
                    </span>
                    {isHolding && (
                      <span className="text-emerald-400 font-mono text-[10px] shrink-0">
                        {formatSmart(Math.abs(level.hold_quantity))}
                      </span>
                    )}
                    {isClosed && (
                      <span className="w-1.5 h-1.5 rounded-full bg-on-surface-muted shrink-0" />
                    )}
                    {!isHolding && !isClosed && (
                      <span className="w-1.5 h-1.5 rounded-full bg-on-surface-faint shrink-0" />
                    )}
                  </div>
                )
              })}
            </div>
          </>
        )}
      </div>
    </div>
  )
}

export default memo(LevelsOverview)
