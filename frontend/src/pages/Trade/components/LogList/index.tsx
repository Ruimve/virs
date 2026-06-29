import { memo } from 'react';
import { useNavigate } from 'react-router-dom';
import type { AnalysisLog } from '@/service';
import { Icon as AiThinkingIcon } from '@/components/Transition/Icon/AiThinking';
import { ChevronRight } from '@/components/Icon';
import { actionColor, actionLabel } from '../utils/utils';

interface Props {
  logs: AnalysisLog[];
  loading: boolean;
  onLoadMore?: () => void;
  botType: 'auto' | 'grid';
  botId: string;
}

const LogList = ({ logs, loading, onLoadMore, botType, botId }: Props) => {
  const navigate = useNavigate();

  const getDecision = (log: AnalysisLog) => {
    if (log.result?.action) return log.result;
    if (log.result?.decision) return log.result.decision;
    return null;
  };

  if (loading && logs.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-16 gap-4 text-on-surface-tertiary text-xs">
        <AiThinkingIcon size={40} />
        <span className="tracking-wider">AI 决策加载中</span>
      </div>
    );
  }

  if (logs.length === 0) {
    return (
      <div className="text-center py-20 text-on-surface-tertiary text-xs">暂无 AI 决策记录</div>
    );
  }

  return (
    <div className="px-4 md:px-8 py-4 space-y-3">
      {logs.map((log) => {
        const decision = getDecision(log);

        return (
          <div
            key={log.id}
            onClick={() => navigate(`/trade/${botType}/${botId}/log/${log.id}`)}
            className="bg-surface-1 rounded-xl border border-line-default shadow-sm overflow-hidden cursor-pointer hover:bg-surface-2/50 transition-colors"
          >
            <div className="flex items-center justify-between px-4 py-3">
              <div className="flex items-center gap-2">
                {decision && (
                  <span
                    className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${actionColor(decision.action!)}`}
                  >
                    {actionLabel(decision.action!)}
                  </span>
                )}
                {log.status === 'failed' && (
                  <span className="text-[10px] font-medium px-1.5 py-0.5 rounded bg-danger-bg text-danger-text">
                    失败
                  </span>
                )}
                <span className="text-[11px] text-on-surface-tertiary">
                  {new Date(log.created_at).toLocaleString('zh-CN')}
                </span>
              </div>
              <ChevronRight className="w-3.5 h-3.5 text-on-surface-muted" strokeWidth={2} />
            </div>
          </div>
        );
      })}

      {onLoadMore && (
        <div className="text-center pt-2">
          <button
            onClick={onLoadMore}
            disabled={loading}
            className="px-4 py-2 rounded-lg text-xs font-medium text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2 transition-colors disabled:opacity-50"
          >
            {loading ? '加载中...' : '加载更多'}
          </button>
        </div>
      )}
    </div>
  );
};

export default memo(LogList);
