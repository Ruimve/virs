import { memo } from 'react';
import { useNavigate } from 'react-router-dom';
import type { AnalysisLog } from '@/service';
import { AiThinking } from '@/components/Transition/Icon';
import Badge from '@/components/Badge';
import StateFeedback from '@/components/StateFeedback';
import { ChevronRight } from '@/components/Icon';
import { actionLabel, actionVariant } from '../utils/utils';
import { Button } from '@/components/Button';

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
    return <StateFeedback type="loading" text="AI 决策加载中" icon={<AiThinking size={40} />} />;
  }

  if (logs.length === 0) {
    return <StateFeedback type="empty" text="暂无 AI 决策记录" />;
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
                  <Badge variant={actionVariant(decision.action!)}>
                    {actionLabel(decision.action!)}
                  </Badge>
                )}
                {log.status === 'failed' && <Badge variant="danger">失败</Badge>}
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
