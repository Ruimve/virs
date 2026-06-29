import { memo } from 'react';
import { useNavigate } from 'react-router-dom';
import type { AnalysisLog } from '@/service';
import { actionColor, actionLabel } from '../../../components/utils/utils';

interface Props {
  logs: AnalysisLog[];
  botId: string;
  botType?: 'auto' | 'grid';
}

const RecentDecisions = ({ logs, botId, botType = 'auto' }: Props) => {
  const navigate = useNavigate();
  const recent = logs.slice(0, 5);

  const getDecision = (log: AnalysisLog) => {
    if (log.result?.action) return log.result;
    if (log.result?.decision) return log.result.decision;
    return null;
  };

  return (
    <div className="flex flex-col min-h-0">
      <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle shrink-0">
        <span className="text-[11px] uppercase tracking-wider text-on-surface-tertiary font-medium">
          最近决策
        </span>
        <span className="text-[11px] font-mono tabular-nums text-on-surface-muted">
          {logs.length}
        </span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {recent.length === 0 ? (
          <div className="text-center py-6 text-sm text-on-surface-tertiary">暂无决策</div>
        ) : (
          <div className="divide-y divide-line-subtle">
            {recent.map((log) => {
              const decision = getDecision(log);
              return (
                <div
                  key={log.id}
                  onClick={() => navigate(`/trade/${botType}/${botId}/log/${log.id}`)}
                  className="px-3 py-2 hover:bg-surface-2/50 cursor-pointer transition-colors"
                >
                  <div className="flex items-center gap-2 mb-1">
                    {decision && (
                      <span
                        className={`text-[11px] font-medium px-1.5 py-0.5 rounded ${actionColor(decision.action!)}`}
                      >
                        {actionLabel(decision.action!)}
                      </span>
                    )}
                    {log.status === 'failed' && (
                      <span className="text-[11px] font-medium px-1.5 py-0.5 rounded bg-danger-bg text-danger-text">
                        失败
                      </span>
                    )}
                    <span className="text-[11px] text-on-surface-muted ml-auto font-mono tabular-nums">
                      {new Date(log.created_at).toLocaleTimeString('zh-CN', {
                        hour: '2-digit',
                        minute: '2-digit',
                      })}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};

export default memo(RecentDecisions);
