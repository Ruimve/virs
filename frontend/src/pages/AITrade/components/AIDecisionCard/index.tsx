import { memo, useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import type { AnalysisLog } from '@/service/types';
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
      <div className="px-4 py-6 text-center text-sm text-on-surface-tertiary">
        等待 AI 首次决策...
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
          <span className="text-[11px] font-medium px-1.5 py-0.5 rounded bg-red-500/10 text-red-400">
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
        <div className="mt-2 flex items-start gap-1.5 text-[11px] text-amber-400">
          <svg
            className="w-3.5 h-3.5 mt-0.5 shrink-0"
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
        <div className="mt-2 text-[11px] text-on-surface-muted">
          模型 <span className="font-mono tabular-nums">{log.llm_model}</span>
        </div>
      )}
    </div>
  );
};

export default memo(AIDecisionCard);
