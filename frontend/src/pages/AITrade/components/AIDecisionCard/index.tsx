import { memo, useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import type { AnalysisLog } from '@/service/types';
import Skeleton from '@/components/Skeleton';
import { Warning } from '@/components/Icon';
import { actionColor, actionLabel, confidenceColor } from '../utils/utils';

interface Props {
  log: AnalysisLog | null;
  botId: string;
  botType?: 'auto' | 'grid';
}

const AIDecisionCard = ({ log, botId, botType = 'auto' }: Props) => {
  const navigate = useNavigate();

  const handleClick = useCallback(() => {
    navigate(`/trade/${botType}/${botId}/log/${log?.id}`);
  }, [botType, botId, log?.id, navigate]);

  const decision = log?.result?.decision;
  const market = log?.result?.market;
  const riskWarning = log?.result?.risk_warning;

  const confidence = useMemo(() => {
    const confidence = log?.result?.decision?.confidence;
    if (typeof confidence === 'number' && !isNaN(confidence)) return confidence;
    return null;
  }, [log?.result?.decision]);

  const createdAt = useMemo(() => {
    if (!log?.created_at) return '-';
    return new Date(log?.created_at).toLocaleTimeString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit',
    });
  }, [log?.created_at]);

  if (!log) {
    return (
      <div className="px-4 py-3 border-b border-line-subtle">
        {/* 标题行 */}
        <div className="flex items-center justify-between mb-2">
          <span className="text-[11px] uppercase tracking-wider text-on-surface-tertiary font-medium">
            AI 决策
          </span>
          <Skeleton className="h-3 w-10" />
        </div>
        {/* Action + Confidence 占位 */}
        <div className="flex items-center gap-2 mb-2">
          <Skeleton className="h-5 w-14" />
          <Skeleton className="h-4 w-8" />
        </div>
        {/* Reason 占位（2 行） */}
        <div className="space-y-1.5">
          <Skeleton className="h-3 w-full" />
          <Skeleton className="h-3 w-3/4" />
        </div>
      </div>
    );
  }

  return (
    <div
      onClick={handleClick}
      className="px-4 py-3 border-b border-line-subtle cursor-pointer hover:bg-surface-2/30 transition-colors"
    >
      {/* 标题行 */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-[11px] uppercase tracking-wider text-on-surface-tertiary font-medium">
          AI 决策
        </span>
        <span className="text-[11px] font-mono tabular-nums text-on-surface-muted">
          {createdAt}
        </span>
      </div>

      {/* Action + Confidence */}
      <div className="flex items-center gap-2 mb-2">
        <span
          className={`text-sm font-medium px-2 py-0.5 rounded border ${actionColor(decision?.action)}`}
        >
          {actionLabel(decision?.action)}
        </span>
        {confidence && (
          <span className={`text-sm font-mono tabular-nums ${confidenceColor(confidence)}`}>
            {(confidence * 100).toFixed(0)}%
          </span>
        )}
        {log.status === 'failed' && (
          <span className="text-[11px] font-medium px-1.5 py-0.5 rounded bg-danger-bg text-danger-text">
            失败
          </span>
        )}
        {market?.regime && (
          <span className="text-[11px] text-on-surface-tertiary ml-auto">
            市况 <span className="text-on-surface font-mono tabular-nums">{market?.regime}</span>
          </span>
        )}
      </div>

      {/* Reason */}
      {decision?.reason && (
        <p className="text-sm text-on-surface-secondary leading-relaxed line-clamp-3">
          {decision?.reason}
        </p>
      )}

      {/* Risk Warning */}
      {riskWarning && (
        <div className="mt-2 flex items-start gap-1.5 text-[11px] text-warning-text">
          <Warning className="w-3.5 h-3.5 mt-0.5 shrink-0" strokeWidth={2} />
          <span className="line-clamp-2">{riskWarning}</span>
        </div>
      )}

      {/* LLM Model */}
      {log.llm_model && (
        <div className="mt-2 text-[11px] text-on-surface-muted">
          模型 <span className="font-mono tabular-nums">{log.llm_model}</span>
        </div>
      )}
    </div>
  );
};

export default memo(AIDecisionCard);
