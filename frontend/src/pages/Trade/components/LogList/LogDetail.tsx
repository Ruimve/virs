import { useState, memo } from 'react';
import { useNavigate } from 'react-router-dom';
import type { AnalysisLog } from '@/service/types';
import { AiThinking } from '../Transition';
import { ChevronLeft, ChevronRight } from '@/components/Icon';
import { Badge } from '@/components/Badge';
import { Card } from '@/components/Card';
import { Progress } from '@/components/Progress';
import { IndicatorChip } from '../IndicatorChip';
import { Radar } from '@/components/Radar';
import { Alert } from '@/components/Alert';
import { IconBtn } from '@/components/Button/IconBtn';
import {
  actionLabel,
  actionVariant,
  executionStatusLabel,
  executionStatusVariant,
} from '../utils/utils';
import {
  getDecision,
  extractIndicatorChips,
  extractRadarData,
  extractTargetPrices,
} from '../utils/logUtils';
import { formatCompact } from '../../ChatBot/Bot/components/utils';

interface Props {
  log: AnalysisLog;
  loading: boolean;
}

const LogDetail = ({ log, loading }: Props) => {
  const navigate = useNavigate();

  const [showSystemPrompt, setShowSystemPrompt] = useState(false);
  const [showUserPrompt, setShowUserPrompt] = useState(false);

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
  const confidence = decision?.confidence ?? 0;
  const confidencePct = confidence * 100;
  const reason = decision?.reason || log.result?.analysis || '';
  const indicators = extractIndicatorChips(log.result?.market);
  const radarData = extractRadarData(log);
  const targets = extractTargetPrices(log);
  const market = log.result?.market;
  const risk = log.result?.risk;

  return (
    <div className="h-full overflow-y-auto bg-base">
      {/* Sticky header */}
      <div className="sticky top-0 z-20 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
        <div className="flex items-center gap-3">
          <IconBtn onClick={() => navigate(-1)}>
            <ChevronLeft className="w-5 h-5" strokeWidth={2} />
          </IconBtn>
          <h1 className="text-sm font-medium text-on-surface">AI 决策详情</h1>
        </div>
      </div>

      <div className="px-4 md:px-8 py-6">
        <div className="space-y-4">
          {/* Decision summary card */}
          <Card>
            <div className="flex items-center gap-3 mb-3">
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
              {log.execution_status && (
                <Badge variant={executionStatusVariant(log.execution_status)} size="sm">
                  {executionStatusLabel(log.execution_status)}
                </Badge>
              )}
              {log.llm_model && (
                <span className="text-2xs font-mono px-2 py-0.5 rounded bg-surface-3 text-on-surface-secondary border border-line-default">
                  {log.llm_model}
                </span>
              )}
              <span className="text-2xs text-on-surface-tertiary font-mono tabular-nums">
                {new Date(log.created_at).toLocaleString('zh-CN')}
              </span>
              <div className="flex-1" />
              {/* Large confidence display */}
              {confidence > 0 && (
                <div className="text-right">
                  <div className="text-2xs text-on-surface-muted uppercase tracking-wider">
                    置信度
                  </div>
                  <div className="text-2xl font-bold font-mono tabular-nums text-ai leading-tight">
                    {confidencePct.toFixed(0)}%
                  </div>
                </div>
              )}
            </div>
            {/* Confidence bar */}
            {confidence > 0 && <Progress pct={confidencePct} color="ai" showValue />}
          </Card>

          {/* Execution result card */}
          {log.execution_status && (
            <Card>
              <div className="text-2xs font-semibold text-on-surface-tertiary uppercase tracking-wider mb-3">
                执行结果
              </div>
              <div className="flex items-center gap-3 flex-wrap">
                <Badge variant={executionStatusVariant(log.execution_status)} size="sm">
                  {executionStatusLabel(log.execution_status)}
                </Badge>
                {log.completed_at && (
                  <span className="text-2xs text-on-surface-tertiary font-mono tabular-nums">
                    {new Date(log.completed_at).toLocaleString('zh-CN')}
                  </span>
                )}
              </div>
              {log.intercept_reason && (
                <div className="mt-3 rounded-lg bg-danger-bg/50 border border-danger-border/50 px-3 py-2">
                  <div className="text-2xs font-semibold text-danger-text uppercase tracking-wider mb-1">
                    未执行原因
                  </div>
                  <p className="text-xs text-on-surface-secondary leading-relaxed">
                    {log.intercept_reason}
                  </p>
                </div>
              )}
            </Card>
          )}

          {/* AI reasoning card with ai-bg */}
          {(reason || indicators.length > 0) && (
            <div className="rounded-xl border border-ai-border bg-ai-bg p-4">
              {reason && (
                <>
                  <div className="text-2xs font-semibold text-ai uppercase tracking-wider mb-2">
                    AI 推理
                  </div>
                  <p className="text-xs text-on-surface-secondary leading-relaxed mb-3">{reason}</p>
                </>
              )}
              {indicators.length > 0 && (
                <div className="flex flex-wrap gap-1.5">
                  {indicators.map((ind) => (
                    <IndicatorChip
                      key={ind.name}
                      name={ind.name}
                      value={ind.value}
                      sentiment={ind.sentiment}
                    />
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Radar chart + Market state (desktop 2-col) */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {/* Radar chart card */}
            <Card>
              <div className="text-2xs font-semibold text-on-surface-tertiary uppercase tracking-wider mb-3">
                指标雷达
              </div>
              <div className="flex items-center justify-center">
                <Radar data={radarData} size={140} />
              </div>
            </Card>

            {/* Market state card */}
            {market && (
              <Card>
                <div className="text-2xs font-semibold text-on-surface-tertiary uppercase tracking-wider mb-3">
                  市场状态
                </div>
                <div className="grid grid-cols-2 gap-2.5">
                  {market.market_regime && (
                    <div>
                      <div className="text-2xs text-on-surface-muted">市场形态</div>
                      <span
                        className={`inline-block mt-0.5 px-2 py-0.5 text-2xs rounded font-medium ${
                          market.market_regime === 'ranging'
                            ? 'bg-info-bg text-info-text'
                            : market.market_regime === 'trending_up'
                              ? 'bg-success-bg text-success-text'
                              : market.market_regime === 'trending_down'
                                ? 'bg-danger-bg text-danger-text'
                                : market.market_regime === 'volatile'
                                  ? 'bg-warning-bg text-warning-text'
                                  : 'bg-surface-2 text-on-surface-secondary'
                        }`}
                      >
                        {market.market_regime}
                      </span>
                    </div>
                  )}
                  {market.rsi != null && (
                    <div>
                      <div className="text-2xs text-on-surface-muted">RSI</div>
                      <div className="text-xs font-mono font-semibold tabular-nums text-on-surface">
                        {Number(market.rsi).toFixed(1)}
                      </div>
                    </div>
                  )}
                  {market.macd != null && (
                    <div>
                      <div className="text-2xs text-on-surface-muted">MACD</div>
                      <div className="text-xs font-mono font-semibold tabular-nums text-on-surface">
                        {Number(market.macd).toFixed(4)}
                      </div>
                    </div>
                  )}
                  {market.ema != null && (
                    <div>
                      <div className="text-2xs text-on-surface-muted">EMA</div>
                      <div className="text-xs font-mono font-semibold tabular-nums text-on-surface">
                        {Number(market.ema).toFixed(2)}
                      </div>
                    </div>
                  )}
                  {market.atr != null && (
                    <div>
                      <div className="text-2xs text-on-surface-muted">ATR</div>
                      <div className="text-xs font-mono font-semibold tabular-nums text-on-surface">
                        {Number(market.atr).toFixed(2)}
                      </div>
                    </div>
                  )}
                  {market.adx != null && (
                    <div>
                      <div className="text-2xs text-on-surface-muted">ADX</div>
                      <div className="text-xs font-mono font-semibold tabular-nums text-on-surface">
                        {Number(market.adx).toFixed(1)}
                      </div>
                    </div>
                  )}
                </div>
                {market.funding_rate_warning && (
                  <p className="text-2xs text-warning-text mt-2">{market.funding_rate_warning}</p>
                )}
                {market.event_impact && (
                  <p className="text-2xs text-ai mt-1">{market.event_impact}</p>
                )}
              </Card>
            )}
          </div>

          {/* Risk parameters card */}
          {(risk || targets.entry || targets.stopLoss || targets.takeProfit) && (
            <Card>
              <div className="text-2xs font-semibold text-on-surface-tertiary uppercase tracking-wider mb-3">
                风控参数
              </div>
              <div className="grid grid-cols-2 md:grid-cols-4 gap-2.5">
                {targets.entry != null && (
                  <div>
                    <div className="text-2xs text-on-surface-muted">入场价</div>
                    <div className="text-xs font-mono font-semibold tabular-nums text-on-surface">
                      {formatCompact(targets.entry)}
                    </div>
                  </div>
                )}
                {targets.stopLoss != null && (
                  <div>
                    <div className="text-2xs text-on-surface-muted">止损</div>
                    <div className="text-xs font-mono font-semibold tabular-nums text-danger-text">
                      {formatCompact(targets.stopLoss)}
                    </div>
                  </div>
                )}
                {targets.takeProfit != null && (
                  <div>
                    <div className="text-2xs text-on-surface-muted">止盈</div>
                    <div className="text-xs font-mono font-semibold tabular-nums text-success-text">
                      {formatCompact(targets.takeProfit)}
                    </div>
                  </div>
                )}
                {risk?.leverage && (
                  <div>
                    <div className="text-2xs text-on-surface-muted">杠杆</div>
                    <div className="text-xs font-mono font-semibold tabular-nums text-warning-text">
                      {risk.leverage}x
                    </div>
                  </div>
                )}
                {risk?.position_size && (
                  <div>
                    <div className="text-2xs text-on-surface-muted">仓位</div>
                    <div className="text-xs font-mono font-semibold tabular-nums text-on-surface">
                      {risk.position_size}
                    </div>
                  </div>
                )}
              </div>
            </Card>
          )}

          {/* Risk warning */}
          {log.result?.risk_warning && log.result.risk_warning !== 'none' && (
            <Alert type="warning" title={log.result.risk_warning} />
          )}

          {/* Error */}
          {log.error && <Alert type="danger" title={log.error} />}

          {/* System Prompt */}
          {log.system_prompt && (
            <Card padding={false} className="overflow-hidden">
              <div
                onClick={() => setShowSystemPrompt(!showSystemPrompt)}
                className="w-full flex items-center justify-between gap-2 px-4 py-3 text-2xs font-mono font-semibold text-on-surface-secondary uppercase tracking-wider hover:text-on-surface transition-colors cursor-pointer"
              >
                System Prompt
                <ChevronRight
                  className={`w-3 h-3 transition-transform ${showSystemPrompt ? 'rotate-90' : ''}`}
                  strokeWidth={2}
                />
              </div>
              {showSystemPrompt && (
                <div className="px-4 pb-4">
                  <pre className="text-caption text-on-surface-tertiary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">
                    {log.system_prompt}
                  </pre>
                </div>
              )}
            </Card>
          )}

          {/* User Prompt */}
          {log.user_prompt && (
            <Card padding={false} className="overflow-hidden">
              <div
                onClick={() => setShowUserPrompt(!showUserPrompt)}
                className="w-full flex items-center justify-between gap-2 px-4 py-3 text-2xs font-mono font-semibold text-on-surface-secondary uppercase tracking-wider hover:text-on-surface transition-colors cursor-pointer"
              >
                User Prompt
                <ChevronRight
                  className={`w-3 h-3 transition-transform ${showUserPrompt ? 'rotate-90' : ''}`}
                  strokeWidth={2}
                />
              </div>
              {showUserPrompt && (
                <div className="px-4 pb-4">
                  <pre className="text-caption text-on-surface-tertiary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">
                    {log.user_prompt}
                  </pre>
                </div>
              )}
            </Card>
          )}

          {/* LLM raw response */}
          {log.result?.raw_llm_response && (
            <Card>
              <div className="text-2xs font-semibold text-on-surface-tertiary uppercase tracking-wider mb-2">
                LLM 原始响应
              </div>
              <pre className="text-caption text-ai bg-ai-bg rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-ai-border">
                {typeof log.result.raw_llm_response === 'string'
                  ? log.result.raw_llm_response
                  : JSON.stringify(log.result.raw_llm_response, null, 2)}
              </pre>
            </Card>
          )}

          {/* Full result fallback */}
          {log.result && !log.result.raw_llm_response && (
            <Card>
              <div className="text-2xs font-semibold text-on-surface-tertiary uppercase tracking-wider mb-2">
                完整结果
              </div>
              <pre className="text-caption text-on-surface-secondary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">
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
