import { memo } from 'react'
import { useNavigate } from 'react-router-dom'
import type { AnalysisLog } from '../../../../service/types'

interface AIDecisionCardProps {
  log: AnalysisLog | null
  botId: string
  botType?: 'auto' | 'grid'
}

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
    open_long: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
    open_short: 'bg-red-500/10 text-red-400 border-red-500/20',
    close_position: 'bg-blue-500/10 text-blue-400 border-blue-500/20',
    hold: 'bg-surface-2 text-on-surface-tertiary border-line-default',
    reduce_position: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
    cancel_order: 'bg-surface-2 text-on-surface-tertiary border-line-default',
  }
  return map[action] || 'bg-surface-2 text-on-surface-tertiary border-line-default'
}

const confidenceColor = (conf: number) => {
  if (conf >= 0.7) return 'text-emerald-400'
  if (conf >= 0.4) return 'text-amber-400'
  return 'text-red-400'
}

const AIDecisionCard = ({ log, botId, botType = 'auto' }: AIDecisionCardProps) => {
  const navigate = useNavigate()

  if (!log) {
    return (
      <div className="px-4 py-6 text-center text-[11px] text-on-surface-tertiary">
        等待 AI 首次决策...
      </div>
    )
  }

  const decision = log.result?.decision || log.result
  const action = decision?.action || 'unknown'
  const reason = decision?.reason
  const confidence = decision?.confidence
  const riskWarning = log.result?.risk_warning
  const marketRegime = log.result?.market?.regime

  return (
    <div
      onClick={() => navigate(`/trade/${botType}/${botId}/log/${log.id}`)}
      className="px-4 py-3 border-b border-line-subtle cursor-pointer hover:bg-surface-2/30 transition-colors"
    >
      {/* 标题行 */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-[10px] text-on-surface-tertiary uppercase tracking-wider">
          AI 决策
        </span>
        <span className="text-[10px] text-on-surface-muted">
          {new Date(log.created_at).toLocaleTimeString('zh-CN', {
            hour: '2-digit',
            minute: '2-digit',
          })}
        </span>
      </div>

      {/* Action + Confidence */}
      <div className="flex items-center gap-2 mb-2">
        <span className={`text-xs font-medium px-2 py-0.5 rounded border ${actionColor(action)}`}>
          {actionLabel(action)}
        </span>
        {confidence !== undefined && confidence !== null && (
          <span className={`text-[11px] font-mono ${confidenceColor(confidence)}`}>
            {(confidence * 100).toFixed(0)}%
          </span>
        )}
        {log.status === 'failed' && (
          <span className="text-[10px] font-medium px-1.5 py-0.5 rounded bg-red-500/10 text-red-400">
            失败
          </span>
        )}
        {marketRegime && (
          <span className="text-[10px] text-on-surface-tertiary ml-auto">
            市况 <span className="text-on-surface font-mono">{marketRegime}</span>
          </span>
        )}
      </div>

      {/* Reason */}
      {reason && (
        <p className="text-[11px] text-on-surface-secondary leading-relaxed line-clamp-3">
          {reason}
        </p>
      )}

      {/* Risk Warning */}
      {riskWarning && (
        <div className="mt-2 flex items-start gap-1.5 text-[10px] text-amber-400">
          <svg
            className="w-3 h-3 mt-0.5 shrink-0"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M12 9v2m0 4h.01M5.07 19h13.86c1.54 0 2.5-1.67 1.73-3L13.73 4c-.77-1.33-2.69-1.33-3.46 0L3.34 16c-.77 1.33.19 3 1.73 3z"
            />
          </svg>
          <span className="line-clamp-2">{riskWarning}</span>
        </div>
      )}

      {/* LLM Model */}
      {log.llm_model && (
        <div className="mt-2 text-[10px] text-on-surface-muted">
          模型 <span className="font-mono">{log.llm_model}</span>
        </div>
      )}
    </div>
  )
}

export default memo(AIDecisionCard)
