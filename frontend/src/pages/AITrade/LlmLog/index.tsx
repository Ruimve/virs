import { useState, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { getAutoAnalysisLogs, getGridAnalysisLogs } from '../../../service/bot'
import type { AnalysisLog } from '../../../service/types'

export default function AnalysisLogDetailPage() {
  const params = useParams()
  const navigate = useNavigate()

  const [log, setLog] = useState<AnalysisLog | null>(null)
  const [loading, setLoading] = useState(true)
  const [showSystemPrompt, setShowSystemPrompt] = useState(false)
  const [showUserPrompt, setShowUserPrompt] = useState(false)

  useEffect(() => {
    const load = async () => {
      try {
        const botType = params.botType
        const res =
          botType === 'auto'
            ? await getAutoAnalysisLogs(params.botId!)
            : await getGridAnalysisLogs(params.botId!)

        const logs =
          botType === 'auto'
            ? (res.data as { logs: AnalysisLog[] } | undefined)?.logs
            : (res.data as { items: AnalysisLog[] } | undefined)?.items
        const found = (logs || []).find((l: AnalysisLog) => l.id === params.logId)
        setLog(found || null)
      } catch (e) {
        console.error('Failed to load analysis log:', e)
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [params.botType, params.botId, params.logId])

  const getDecision = (log: AnalysisLog) => {
    if (log.result?.decision) return log.result.decision
    return null
  }

  const getActionLabel = (action: string) => {
    switch (action) {
      case 'open_long':
        return '开多'
      case 'open_short':
        return '开空'
      case 'close_position':
        return '平仓'
      case 'hold':
        return '持有'
      case 'adjust_grid':
        return '调整网格'
      case 'pause_grid':
        return '暂停网格'
      case 'resume_grid':
        return '恢复网格'
      case 'reduce_position':
        return '减仓'
      case 'cancel_order':
        return '取消订单'
      default:
        return action
    }
  }

  const getActionColor = (action: string) => {
    switch (action) {
      case 'open_long':
        return 'bg-emerald-500/10 text-emerald-400'
      case 'open_short':
        return 'bg-red-500/10 text-red-400'
      case 'close_position':
        return 'bg-blue-500/10 text-blue-400'
      case 'hold':
        return 'bg-surface-2 text-on-surface-tertiary'
      case 'adjust_grid':
        return 'bg-blue-500/10 text-blue-400'
      case 'pause_grid':
        return 'bg-red-500/10 text-red-400'
      case 'resume_grid':
        return 'bg-emerald-500/10 text-emerald-400'
      case 'reduce_position':
        return 'bg-amber-500/10 text-amber-400'
      default:
        return 'bg-surface-2 text-on-surface-tertiary'
    }
  }

  if (loading) {
    return (
      <div className="min-h-screen bg-base">
        <div className="sticky top-0 z-20 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
          <div className="flex items-center gap-3">
            <button
              onClick={() => navigate(-1)}
              className="p-1.5 rounded-lg hover:bg-surface-2 transition-colors text-on-surface-tertiary hover:text-on-surface"
            >
              <svg
                className="w-5 h-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
              </svg>
            </button>
            <h1 className="text-sm font-medium text-on-surface">AI 决策详情</h1>
          </div>
        </div>
        <div className="text-center py-20 text-on-surface-tertiary text-sm">加载中...</div>
      </div>
    )
  }

  if (!log) {
    return (
      <div className="min-h-screen bg-base">
        <div className="sticky top-0 z-20 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
          <div className="flex items-center gap-3">
            <button
              onClick={() => navigate(-1)}
              className="p-1.5 rounded-lg hover:bg-surface-2 transition-colors text-on-surface-tertiary hover:text-on-surface"
            >
              <svg
                className="w-5 h-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
              </svg>
            </button>
            <h1 className="text-sm font-medium text-on-surface">AI 决策详情</h1>
          </div>
        </div>
        <div className="text-center py-20 text-on-surface-tertiary text-sm">未找到该记录</div>
      </div>
    )
  }

  const decision = getDecision(log)

  return (
    <div className="min-h-screen bg-base">
      {/* Top bar */}
      <div className="sticky top-0 z-20 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
        <div className="flex items-center gap-3">
          <button
            onClick={() => navigate(-1)}
            className="p-1.5 rounded-lg hover:bg-surface-2 transition-colors text-on-surface-tertiary hover:text-on-surface"
          >
            <svg
              className="w-5 h-5"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <h1 className="text-sm font-medium text-on-surface">AI 决策详情</h1>
        </div>
      </div>

      <div className="max-w-3xl mx-auto px-4 md:px-8 py-6">
        <div className="space-y-4">
          {/* Header */}
          <div className="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-2">
                {decision && (
                  <span
                    className={`text-xs font-medium px-2 py-0.5 rounded ${getActionColor(decision.action)}`}
                  >
                    {getActionLabel(decision.action)}
                  </span>
                )}
                {log.status === 'failed' && (
                  <span className="text-xs font-medium px-2 py-0.5 rounded bg-red-500/10 text-red-400">
                    失败
                  </span>
                )}
              </div>
              <span className="text-[11px] text-on-surface-tertiary">
                {new Date(log.created_at).toLocaleString('zh-CN')}
              </span>
            </div>
            {log.llm_model && (
              <div className="text-[11px] text-on-surface-tertiary mb-2">
                模型: <span className="text-indigo-400 font-mono">{log.llm_model}</span>
              </div>
            )}
            {decision?.confidence != null && (
              <div className="text-[11px] text-on-surface-tertiary">
                置信度:{' '}
                <span className="text-on-surface-secondary">
                  {(decision.confidence * 100).toFixed(0)}%
                </span>
              </div>
            )}
          </div>

          {/* Decision */}
          {decision && (
            <div className="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
              <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">
                决策
              </div>
              {decision.reason && (
                <p className="text-xs text-on-surface-secondary leading-relaxed">
                  {decision.reason}
                </p>
              )}
            </div>
          )}

          {/* Analysis */}
          {log.result?.analysis && (
            <div className="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
              <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">
                AI 分析
              </div>
              <p className="text-xs text-on-surface-secondary leading-relaxed whitespace-pre-wrap">
                {log.result.analysis}
              </p>
            </div>
          )}

          {/* Market */}
          {log.result?.market && (
            <div className="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
              <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">
                市场状态
              </div>
              <div className="space-y-1">
                {log.result.market.market_regime && (
                  <span
                    className={`px-2 py-0.5 text-xs rounded font-medium ${
                      log.result.market.market_regime === 'ranging'
                        ? 'bg-blue-500/10 text-blue-400'
                        : log.result.market.market_regime === 'trending_up'
                          ? 'bg-emerald-500/10 text-emerald-400'
                          : log.result.market.market_regime === 'trending_down'
                            ? 'bg-red-500/10 text-red-400'
                            : log.result.market.market_regime === 'volatile'
                              ? 'bg-amber-500/10 text-amber-400'
                              : 'bg-surface-2 text-on-surface-secondary'
                    }`}
                  >
                    {log.result.market.market_regime}
                  </span>
                )}
                {log.result.market.funding_rate_warning && (
                  <p className="text-xs text-amber-400">
                    ⚠ 资金费率: {log.result.market.funding_rate_warning}
                  </p>
                )}
                {log.result.market.event_impact && (
                  <p className="text-xs text-purple-400">
                    事件影响: {log.result.market.event_impact}
                  </p>
                )}
              </div>
            </div>
          )}

          {/* Grid params (for grid bot) */}
          {log.result?.grid && (log.result.grid.upper_price || log.result.grid.lower_price) && (
            <div className="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
              <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">
                网格参数
              </div>
              <div className="flex flex-wrap gap-3 text-xs text-on-surface-secondary">
                {log.result.grid.upper_price && <span>上界 {log.result.grid.upper_price}</span>}
                {log.result.grid.lower_price && <span>下界 {log.result.grid.lower_price}</span>}
                {log.result.grid.grid_count && <span>层数 {log.result.grid.grid_count}</span>}
                {log.result.grid.grid_profit_pct && (
                  <span>利润率 {log.result.grid.grid_profit_pct}%</span>
                )}
              </div>
            </div>
          )}

          {/* Risk params (for grid bot) */}
          {log.result?.risk && (log.result.risk.leverage || log.result.risk.quantity_per_grid) && (
            <div className="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
              <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">
                风控参数
              </div>
              <div className="flex flex-wrap gap-3 text-xs text-on-surface-secondary">
                {log.result.risk.leverage && <span>杠杆 {log.result.risk.leverage}x</span>}
                {log.result.risk.quantity_per_grid && (
                  <span>每格 {log.result.risk.quantity_per_grid} USDT</span>
                )}
              </div>
            </div>
          )}

          {/* Risk warning */}
          {log.result?.risk_warning && log.result.risk_warning !== 'none' && (
            <div className="bg-amber-500/5 rounded-xl border border-amber-500/20 p-5">
              <div className="text-[10px] text-amber-400 uppercase tracking-wider mb-2">
                风险提示
              </div>
              <p className="text-xs text-amber-400">{log.result.risk_warning}</p>
            </div>
          )}

          {/* Error */}
          {log.error && (
            <div className="bg-red-500/5 rounded-xl border border-red-500/20 p-5">
              <div className="text-[10px] text-red-400 uppercase tracking-wider mb-2">错误</div>
              <pre className="text-xs text-red-400 whitespace-pre-wrap">{log.error}</pre>
            </div>
          )}

          {/* System Prompt (collapsible) */}
          {log.system_prompt && (
            <div className="bg-surface-1 rounded-xl border border-line-default shadow-sm overflow-hidden">
              <button
                onClick={() => setShowSystemPrompt(!showSystemPrompt)}
                className="w-full flex items-center gap-2 px-5 py-3 text-[10px] text-on-surface-tertiary uppercase tracking-wider hover:text-on-surface-secondary transition-colors"
              >
                <svg
                  className={`w-3 h-3 transition-transform ${showSystemPrompt ? 'rotate-90' : ''}`}
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
                </svg>
                System Prompt
              </button>
              {showSystemPrompt && (
                <div className="px-5 pb-4">
                  <pre className="text-[11px] text-on-surface-secondary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">
                    {log.system_prompt}
                  </pre>
                </div>
              )}
            </div>
          )}

          {/* User Prompt (collapsible) */}
          {log.user_prompt && (
            <div className="bg-surface-1 rounded-xl border border-line-default shadow-sm overflow-hidden">
              <button
                onClick={() => setShowUserPrompt(!showUserPrompt)}
                className="w-full flex items-center gap-2 px-5 py-3 text-[10px] text-on-surface-tertiary uppercase tracking-wider hover:text-on-surface-secondary transition-colors"
              >
                <svg
                  className={`w-3 h-3 transition-transform ${showUserPrompt ? 'rotate-90' : ''}`}
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
                </svg>
                User Prompt
              </button>
              {showUserPrompt && (
                <div className="px-5 pb-4">
                  <pre className="text-[11px] text-on-surface-secondary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">
                    {log.user_prompt}
                  </pre>
                </div>
              )}
            </div>
          )}

          {/* Raw LLM Response */}
          {log.result?.raw_llm_response && (
            <div className="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
              <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">
                LLM 原始响应
              </div>
              <pre className="text-[11px] text-indigo-400 bg-indigo-500/5 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-indigo-500/10">
                {typeof log.result.raw_llm_response === 'string'
                  ? log.result.raw_llm_response
                  : JSON.stringify(log.result.raw_llm_response, null, 2)}
              </pre>
            </div>
          )}

          {/* Full result JSON */}
          {log.result && !log.result.raw_llm_response && (
            <div className="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
              <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">
                完整结果
              </div>
              <pre className="text-[11px] text-on-surface-secondary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">
                {JSON.stringify(log.result, null, 2)}
              </pre>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
