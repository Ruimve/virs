import { memo } from 'react';
import { useNavigate } from 'react-router-dom';
import type { AnalysisLog } from '@/service';
import { Panel } from '@/components/Panel';
import { Badge } from '@/components/Badge';
import { actionVariant, actionLabel } from '../../../components/utils/utils';

interface Props {
  logs: AnalysisLog[];
  botId: string;
  botType?: 'auto' | 'grid';
}

export const RecentDecisions = memo(({ logs, botId, botType = 'auto' }: Props) => {
  const navigate = useNavigate();
  const recent = logs.slice(0, 5);

  const getDecision = (log: AnalysisLog) => {
    if (log.result?.action) return log.result;
    if (log.result?.decision) return log.result.decision;
    return null;
  };

  return (
    <Panel
      title="最近决策"
      count={logs.length}
      empty={recent.length === 0}
      emptyText="暂无决策"
    >
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
                <Badge variant={actionVariant(decision.action!)} size="sm">
                  {actionLabel(decision.action!)}
                </Badge>
              )}
              {log.status === 'failed' && (
                <Badge variant="danger" size="sm">
                  失败
                </Badge>
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
    </Panel>
  );
});
