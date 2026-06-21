import { memo } from 'react'
import { useNavigate } from 'react-router-dom'
import { formatPnl } from '../shared'
import type { AnalysisLog, AutoTrade } from '../../../../service/types'

// ── 决策标签 ───────────────────────────────────────────────

const actionLabel = (action: string) => {
  const map: Record<string, string> = {
    open_long: '开多',
    open_short: '开空',
    close_position: '平仓',
    hold: '持有',
    reduce_position: '减仓',
    cancel_order: '取消',
  }
  return map[action] || action
}

const actionColor = (action: string) => {
  const map: Record<string, string> = {
    open_long: 'bg-emerald-500/10 text-emerald-400',
    open_short: 'bg-red-500/10 text-red-400',
    close_position: 'bg-blue-500/10 text-blue-400',
    hold: 'bg-surface-2 text-on-surface-tertiary',
    reduce_position: 'bg-amber-500/10 text-amber-400',
    cancel_order: 'bg-surface-2 text-on-surface-tertiary',
  }
  return map[action] || 'bg-surface-2 text-on-surface-tertiary'
}

const tradeTypeLabel = (t: string) => {
  const map: Record<string, string> = {
    open_long: '开多',
    open_short: '开空',
    close_long: '平多',
    close_short: '平空',
    stop_loss: '止损',
    take_profit: '止盈',
  }
  return map[t] || t
}

const tradeTypeColor = (t: string) => {
  if (t.startsWith('open_long') || t === 'close_short') return 'text-emerald-400'
  if (t.startsWith('open_short') || t === 'close_long') return 'text-red-400'
  if (t === 'stop_loss') return 'text-red-400'
  if (t === 'take_profit') return 'text-emerald-400'
  return 'text-on-surface-tertiary'
}

// ── 决策日志面板 ───────────────────────────────────────────

interface RecentDecisionsProps {
  logs: AnalysisLog[]
  botId: string
  botType?: 'auto' | 'grid'
}

const RecentDecisions = memo(({ logs, botId, botType = 'auto' }: RecentDecisionsProps) => {
  const navigate = useNavigate()
  const recent = logs.slice(0, 5)

  const getDecision = (log: AnalysisLog) => {
    if (log.result?.action) return log.result
    if (log.result?.decision) return log.result.decision
    return null
  }

  return (
    <div className="flex flex-col min-h-0">
      <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle shrink-0">
        <span className="text-[10px] text-on-surface-tertiary uppercase tracking-wider">
          最近决策
        </span>
        <span className="text-[10px] text-on-surface-muted">{logs.length}</span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {recent.length === 0 ? (
          <div className="text-center py-6 text-[11px] text-on-surface-tertiary">暂无决策</div>
        ) : (
          <div className="divide-y divide-line-subtle">
            {recent.map((log) => {
              const decision = getDecision(log)
              return (
                <div
                  key={log.id}
                  onClick={() => navigate(`/trade/${botType}/${botId}/log/${log.id}`)}
                  className="px-3 py-2 hover:bg-surface-2/50 cursor-pointer transition-colors"
                >
                  <div className="flex items-center gap-2 mb-1">
                    {decision && (
                      <span
                        className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${actionColor(decision.action!)}`}
                      >
                        {actionLabel(decision.action!)}
                      </span>
                    )}
                    {log.status === 'failed' && (
                      <span className="text-[10px] font-medium px-1.5 py-0.5 rounded bg-red-500/10 text-red-400">
                        失败
                      </span>
                    )}
                    <span className="text-[10px] text-on-surface-tertiary ml-auto">
                      {new Date(log.created_at).toLocaleTimeString('zh-CN', {
                        hour: '2-digit',
                        minute: '2-digit',
                      })}
                    </span>
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
})

// ── 最近成交面板 ───────────────────────────────────────────

interface RecentTradesProps {
  trades: AutoTrade[]
}

const RecentTrades = memo(({ trades }: RecentTradesProps) => {
  const recent = trades.slice(0, 5)

  return (
    <div className="flex flex-col min-h-0">
      <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle shrink-0">
        <span className="text-[10px] text-on-surface-tertiary uppercase tracking-wider">
          最近成交
        </span>
        <span className="text-[10px] text-on-surface-muted">{trades.length}</span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {recent.length === 0 ? (
          <div className="text-center py-6 text-[11px] text-on-surface-tertiary">暂无成交</div>
        ) : (
          <div className="divide-y divide-line-subtle">
            {recent.map((t) => (
              <div key={t.id} className="px-3 py-2">
                <div className="flex items-center justify-between mb-0.5">
                  <span
                    className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${tradeTypeColor(t.trade_type)} bg-surface-2`}
                  >
                    {tradeTypeLabel(t.trade_type)}
                  </span>
                  <span className="text-[10px] text-on-surface-tertiary">
                    {new Date(t.created_at).toLocaleTimeString('zh-CN', {
                      hour: '2-digit',
                      minute: '2-digit',
                    })}
                  </span>
                </div>
                <div className="flex items-center justify-between text-[11px]">
                  <span className="text-on-surface font-mono">
                    {t.side === 'buy' ? '买' : '卖'} {t.quantity.toFixed(4)} @ {t.price.toFixed(2)}
                  </span>
                  {t.pnl !== 0 && <span className="font-mono">{formatPnl(t.pnl)}</span>}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
})

// ── 组合侧边栏 ─────────────────────────────────────────────

interface SidebarProps {
  logs: AnalysisLog[]
  trades: AutoTrade[]
  botId: string
  botType?: 'auto' | 'grid'
}

const BotSidebar = ({ logs, trades, botId, botType }: SidebarProps) => (
  <div className="flex flex-col h-full divide-y divide-line-subtle">
    <div className="flex-1 min-h-0">
      <RecentDecisions logs={logs} botId={botId} botType={botType} />
    </div>
    <div className="flex-1 min-h-0">
      <RecentTrades trades={trades} />
    </div>
  </div>
)

export default BotSidebar
