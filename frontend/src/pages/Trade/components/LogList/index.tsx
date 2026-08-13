import { memo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type { AnalysisLog } from '@/service';
import { AiThinking } from '../Transition';
import { Badge } from '@/components/Badge';
import { StateFeedback } from '@/components/StateFeedback';
import { ChevronRight } from '@/components/Icon';
import { ConfidenceBar } from '@/components/ConfidenceBar';
import { IndicatorChip } from '../IndicatorChip';
import {
  actionLabel,
  actionVariant,
  executionStatusLabel,
  executionStatusVariant,
} from '../utils/utils';
import { getDecision, extractIndicatorChips } from '../utils/logUtils';
import { formatRelativeTime } from '../../AutoBot/Bot/components/utils';
import { Button } from '@/components/Button';

interface Props {
  logs: AnalysisLog[];
  loading: boolean;
  onLoadMore?: () => void;
  botId: string;
  total?: number;
}

const LogList = ({ logs, loading, onLoadMore, botId, total }: Props) => {
  const navigate = useNavigate();
  const [expandedId, setExpandedId] = useState<string | null>(null);

  if (loading && logs.length === 0) {
    return <StateFeedback type="loading" text="AI 决策加载中" icon={<AiThinking size={40} />} />;
  }

  if (logs.length === 0) {
    return <StateFeedback type="empty" text="暂无 AI 决策记录" />;
  }

  return (
    <div className="px-4 md:px-8 py-4">
      {/* Title row */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <div className="text-sm font-semibold text-on-surface">AI 决策日志</div>
          {total != null && (
            <div className="text-2xs text-on-surface-tertiary font-mono mt-0.5">
              共 {total} 条记录
            </div>
          )}
        </div>
      </div>

      <div className="space-y-2.5">
        {logs.map((log) => {
          const decision = getDecision(log);
          const isExpanded = expandedId === log.id;
          const confidence = decision?.confidence ?? 0;
          const confidencePct = confidence * 100;
          const reason = decision?.reason || log.result?.analysis || '';
          const indicators = extractIndicatorChips(log.result?.market);

          return (
            <div
              key={log.id}
              className={`rounded-xl border overflow-hidden transition-colors ${
                isExpanded
                  ? 'border-ai-border bg-surface-1'
                  : 'border-line-default bg-surface-1 hover:bg-surface-2/50'
              }`}
            >
              {/* Header - Desktop: single row with reason preview filling horizontal space */}
              <div
                onClick={() => setExpandedId(isExpanded ? null : log.id)}
                className="hidden md:flex items-center gap-2.5 px-4 py-3 cursor-pointer"
              >
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
                <span className="text-2xs text-on-surface-tertiary font-mono tabular-nums shrink-0">
                  {new Date(log.created_at).toLocaleString('zh-CN')}
                </span>
                {reason ? (
                  <span className="flex-1 min-w-0 text-xs text-on-surface-tertiary truncate">
                    {reason}
                  </span>
                ) : (
                  <div className="flex-1" />
                )}
                {confidence > 0 && (
                  <div className="flex items-center gap-1.5 shrink-0 w-28">
                    <ConfidenceBar value={confidencePct} showValue={false} />
                    <span className="text-xs font-mono font-semibold tabular-nums text-ai shrink-0">
                      {confidencePct.toFixed(0)}%
                    </span>
                  </div>
                )}
                <ChevronRight
                  className={`w-3.5 h-3.5 text-on-surface-muted transition-transform shrink-0 ${
                    isExpanded ? 'rotate-90' : ''
                  }`}
                  strokeWidth={2}
                />
              </div>

              {/* Header - Mobile: two rows, badges + relative time on row 1, confidence bar on row 2 */}
              <div
                onClick={() => setExpandedId(isExpanded ? null : log.id)}
                className="md:hidden px-4 py-3 cursor-pointer"
              >
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
                  {log.execution_status && (
                    <Badge variant={executionStatusVariant(log.execution_status)} size="sm">
                      {executionStatusLabel(log.execution_status)}
                    </Badge>
                  )}
                  <div className="flex-1" />
                  <span className="text-2xs text-on-surface-tertiary font-mono tabular-nums shrink-0">
                    {formatRelativeTime(log.created_at)}
                  </span>
                  <ChevronRight
                    className={`w-3.5 h-3.5 text-on-surface-muted transition-transform shrink-0 ${
                      isExpanded ? 'rotate-90' : ''
                    }`}
                    strokeWidth={2}
                  />
                </div>
                {confidence > 0 && (
                  <div className="flex items-center gap-2 mt-2">
                    <span className="text-2xs text-on-surface-muted shrink-0">置信度</span>
                    <ConfidenceBar value={confidencePct} showValue={false} />
                    <span className="text-xs font-mono font-semibold tabular-nums text-ai shrink-0">
                      {confidencePct.toFixed(0)}%
                    </span>
                  </div>
                )}
              </div>

              {/* Expanded content */}
              {isExpanded && (
                <div className="border-t border-ai-border px-4 pb-4 pt-3">
                  {/* Intercept reason */}
                  {log.intercept_reason && (
                    <div className="mb-3 rounded-lg bg-danger-bg/50 border border-danger-border/50 px-3 py-2">
                      <div className="text-2xs font-semibold text-danger-text uppercase tracking-wider mb-1">
                        未执行原因
                      </div>
                      <p className="text-2xs text-on-surface-secondary leading-relaxed">
                        {log.intercept_reason}
                      </p>
                    </div>
                  )}

                  {/* AI reasoning */}
                  {reason && (
                    <div className="mb-3">
                      <div className="text-2xs font-semibold text-ai uppercase tracking-wider mb-1.5">
                        AI 推理
                      </div>
                      <p className="text-2xs text-on-surface-secondary leading-relaxed line-clamp-4">
                        {reason}
                      </p>
                    </div>
                  )}

                  {/* Indicator chips */}
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

                  {/* Navigate to detail */}
                  <div
                    onClick={(e) => {
                      e.stopPropagation();
                      navigate(`/trade/auto/${botId}/log/${log.id}`);
                    }}
                    className="mt-3 flex items-center gap-1 text-2xs text-ai hover:text-ai-hover cursor-pointer transition-colors"
                  >
                    查看完整详情
                    <ChevronRight className="w-3 h-3" strokeWidth={2} />
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {onLoadMore && (
        <div className="text-center pt-4">
          <Button
            variant="ghost"
            size="xs"
            responsive={false}
            onClick={onLoadMore}
            disabled={loading}
          >
            {loading ? '加载中...' : '加载更多'}
          </Button>
        </div>
      )}
    </div>
  );
};

export default memo(LogList);
