import { useState, memo } from 'react';
import { useNavigate } from 'react-router-dom';
import type { AnalysisLog } from '@/service/types';
import { AiThinking } from '@/components/Transition/Icon';
import { ChevronLeft, ChevronRight } from '@/components/Icon';
import { Badge } from '@/components/Badge';
import { Card } from '@/components/Card';
import { Title } from '@/components/Title';
import { IconBtn } from '@/components/Button/IconBtn';
import { actionLabel, actionVariant } from '../utils/utils';

interface Props {
  log: AnalysisLog;
  loading: boolean;
}

const LogDetail = ({ log, loading }: Props) => {
  const navigate = useNavigate();

  const [showSystemPrompt, setShowSystemPrompt] = useState(false);
  const [showUserPrompt, setShowUserPrompt] = useState(false);

  const getDecision = (log: AnalysisLog) => {
    if (log.result?.decision) return log.result.decision;
    return null;
  };

  if (loading) {
    return (
      <div className="min-h-screen bg-base">
        <div className="sticky top-0 z-20 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
          <div className="flex items-center gap-3">
            <IconBtn onClick={() => navigate(-1)}>
              <ChevronLeft className="w-5 h-5" strokeWidth={2} />
            </IconBtn>
            <h1 className="text-sm font-medium text-on-surface">AI 决策详情</h1>
          </div>
        </div>
        <div className="flex flex-col items-center justify-center py-20 gap-4 text-on-surface-tertiary text-xs">
          <AiThinking size={48} />
          <span className="tracking-wider">AI 决策详情加载中</span>
        </div>
      </div>
    );
  }

  if (!log) {
    return (
      <div className="min-h-screen bg-base">
        <div className="sticky top-0 z-20 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
          <div className="flex items-center gap-3">
            <IconBtn onClick={() => navigate(-1)}>
              <ChevronLeft className="w-5 h-5" strokeWidth={2} />
            </IconBtn>
            <h1 className="text-sm font-medium text-on-surface">AI 决策详情</h1>
          </div>
        </div>
        <div className="text-center py-20 text-on-surface-tertiary text-sm">未找到该记录</div>
      </div>
    );
  }

  const decision = getDecision(log);

  return (
    <div className="h-full overflow-y-auto bg-base">
      {/* Top bar */}
      <div className="sticky top-0 z-20 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
        <div className="flex items-center gap-3">
          <IconBtn onClick={() => navigate(-1)}>
            <ChevronLeft className="w-5 h-5" strokeWidth={2} />
          </IconBtn>
          <h1 className="text-sm font-medium text-on-surface">AI 决策详情</h1>
        </div>
      </div>

      <div className="max-w-3xl mx-auto px-4 md:px-8 py-6">
        <div className="space-y-4">
          {/* Header */}
          <Card>
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-2">
                {decision && (
                  <Badge variant={actionVariant(decision.action)} size="sm">
                    {actionLabel(decision.action)}
                  </Badge>
                )}
                {log.status === 'failed' && (
                  <Badge variant="danger" size="sm">
                    失败
                  </Badge>
                )}
              </div>
              <span className="text-[11px] text-on-surface-tertiary">
                {new Date(log.created_at).toLocaleString('zh-CN')}
              </span>
            </div>
            {log.llm_model && (
              <div className="text-[11px] text-on-surface-tertiary mb-2">
                模型: <span className="text-accent font-mono">{log.llm_model}</span>
              </div>
            )}
            {decision?.confidence != null && (
              <div className="text-[11px] text-on-surface-tertiary">
                置信度:{' '}
                <span className="text-on-surface-secondary font-mono">
                  {(decision.confidence * 100).toFixed(0)}%
                </span>
              </div>
            )}
          </Card>

          {/* Decision */}
          {decision && (
            <Card>
              <Title className="mb-2">决策</Title>
              {decision.reason && (
                <p className="text-xs text-on-surface-secondary leading-relaxed">
                  {decision.reason}
                </p>
              )}
            </Card>
          )}

          {/* Analysis */}
          {log.result?.analysis && (
            <Card>
              <Title className="mb-2">AI 分析</Title>
              <p className="text-xs text-on-surface-secondary leading-relaxed whitespace-pre-wrap">
                {log.result.analysis}
              </p>
            </Card>
          )}

          {/* Market */}
          {log.result?.market && (
            <Card>
              <Title className="mb-2">市场状态</Title>
              <div className="space-y-1">
                {log.result.market.market_regime && (
                  <span
                    className={`px-2 py-0.5 text-xs rounded font-medium ${
                      log.result.market.market_regime === 'ranging'
                        ? 'bg-info-bg text-info-text'
                        : log.result.market.market_regime === 'trending_up'
                          ? 'bg-success-bg text-success-text'
                          : log.result.market.market_regime === 'trending_down'
                            ? 'bg-danger-bg text-danger-text'
                            : log.result.market.market_regime === 'volatile'
                              ? 'bg-warning-bg text-warning-text'
                              : 'bg-surface-2 text-on-surface-secondary'
                    }`}
                  >
                    {log.result.market.market_regime}
                  </span>
                )}
                {log.result.market.funding_rate_warning && (
                  <p className="text-xs text-warning-text">
                    ⚠ 资金费率: {log.result.market.funding_rate_warning}
                  </p>
                )}
                {log.result.market.event_impact && (
                  <p className="text-xs text-accent">事件影响: {log.result.market.event_impact}</p>
                )}
              </div>
            </Card>
          )}

          {/* Grid params (for grid bot) */}
          {log.result?.grid && (log.result.grid.upper_price || log.result.grid.lower_price) && (
            <Card>
              <Title className="mb-2">网格参数</Title>
              <div className="flex flex-wrap gap-3 text-xs text-on-surface-secondary">
                {log.result.grid.upper_price && <span>上界 {log.result.grid.upper_price}</span>}
                {log.result.grid.lower_price && <span>下界 {log.result.grid.lower_price}</span>}
                {log.result.grid.grid_count && <span>层数 {log.result.grid.grid_count}</span>}
                {log.result.grid.grid_profit_pct && (
                  <span>利润率 {log.result.grid.grid_profit_pct}%</span>
                )}
              </div>
            </Card>
          )}

          {/* Risk params (for grid bot) */}
          {log.result?.risk && (log.result.risk.leverage || log.result.risk.quantity_per_grid) && (
            <Card>
              <Title className="mb-2">风控参数</Title>
              <div className="flex flex-wrap gap-3 text-xs text-on-surface-secondary">
                {log.result.risk.leverage && <span>杠杆 {log.result.risk.leverage}x</span>}
                {log.result.risk.quantity_per_grid && (
                  <span>每格 {log.result.risk.quantity_per_grid} USDT</span>
                )}
              </div>
            </Card>
          )}

          {/* Risk warning */}
          {log.result?.risk_warning && log.result.risk_warning !== 'none' && (
            <div className="bg-warning/5 rounded-xl border border-warning-border p-5">
              <div className="text-[10px] text-warning-text uppercase tracking-wider mb-2">
                风险提示
              </div>
              <p className="text-xs text-warning-text">{log.result.risk_warning}</p>
            </div>
          )}

          {/* Error */}
          {log.error && (
            <div className="bg-danger/5 rounded-xl border border-danger-border p-5">
              <div className="text-[10px] text-danger-text uppercase tracking-wider mb-2">错误</div>
              <pre className="text-xs text-danger-text whitespace-pre-wrap">{log.error}</pre>
            </div>
          )}

          {/* System Prompt (collapsible) */}
          {log.system_prompt && (
            <Card padding={false} className="overflow-hidden">
              <div
                onClick={() => setShowSystemPrompt(!showSystemPrompt)}
                className="w-full flex items-center gap-2 px-5 py-3 text-[10px] text-on-surface-tertiary uppercase tracking-wider hover:text-on-surface-secondary transition-colors cursor-pointer"
              >
                <ChevronRight
                  className={`w-3 h-3 transition-transform ${showSystemPrompt ? 'rotate-90' : ''}`}
                  strokeWidth={2}
                />
                System Prompt
              </div>
              {showSystemPrompt && (
                <div className="px-5 pb-4">
                  <pre className="text-[11px] text-on-surface-secondary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">
                    {log.system_prompt}
                  </pre>
                </div>
              )}
            </Card>
          )}

          {/* User Prompt (collapsible) */}
          {log.user_prompt && (
            <Card padding={false} className="overflow-hidden">
              <div
                onClick={() => setShowUserPrompt(!showUserPrompt)}
                className="w-full flex items-center gap-2 px-5 py-3 text-[10px] text-on-surface-tertiary uppercase tracking-wider hover:text-on-surface-secondary transition-colors cursor-pointer"
              >
                <ChevronRight
                  className={`w-3 h-3 transition-transform ${showUserPrompt ? 'rotate-90' : ''}`}
                  strokeWidth={2}
                />
                User Prompt
              </div>
              {showUserPrompt && (
                <div className="px-5 pb-4">
                  <pre className="text-[11px] text-on-surface-secondary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">
                    {log.user_prompt}
                  </pre>
                </div>
              )}
            </Card>
          )}

          {/* Raw LLM Response */}
          {log.result?.raw_llm_response && (
            <Card>
              <Title className="mb-2">LLM 原始响应</Title>
              <pre className="text-[11px] text-accent bg-accent/5 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-accent/10">
                {typeof log.result.raw_llm_response === 'string'
                  ? log.result.raw_llm_response
                  : JSON.stringify(log.result.raw_llm_response, null, 2)}
              </pre>
            </Card>
          )}

          {/* Full result JSON */}
          {log.result && !log.result.raw_llm_response && (
            <Card>
              <Title className="mb-2">完整结果</Title>
              <pre className="text-[11px] text-on-surface-secondary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">
                {JSON.stringify(log.result, null, 2)}
              </pre>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
};

export default memo(LogDetail);
