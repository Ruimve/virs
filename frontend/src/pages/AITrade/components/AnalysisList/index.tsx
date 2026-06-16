import { useNavigate } from 'react-router-dom';
import type { AnalysisLog } from '../shared';

interface AnalysisListProps {
  logs: AnalysisLog[];
  loading: boolean;
  onLoadMore?: () => void;
  botType: 'auto' | 'grid';
  botId: string;
}

export default function AnalysisList({ logs, loading, onLoadMore, botType, botId }: AnalysisListProps) {
  const navigate = useNavigate();

  const getActionLabel = (action: string) => {
    const map: Record<string, string> = {
      open_long: '开多', open_short: '开空', close_position: '平仓',
      hold: '持有', adjust_grid: '调整网格', pause_grid: '暂停网格',
      resume_grid: '恢复网格', reduce_position: '减仓', cancel_order: '取消订单',
    };
    return map[action] || action;
  };

  const getActionColor = (action: string) => {
    const map: Record<string, string> = {
      open_long: 'bg-emerald-500/10 text-emerald-400',
      open_short: 'bg-red-500/10 text-red-400',
      close_position: 'bg-blue-500/10 text-blue-400',
      hold: 'bg-surface-2 text-on-surface-tertiary',
      adjust_grid: 'bg-blue-500/10 text-blue-400',
      pause_grid: 'bg-red-500/10 text-red-400',
      resume_grid: 'bg-emerald-500/10 text-emerald-400',
      reduce_position: 'bg-amber-500/10 text-amber-400',
    };
    return map[action] || 'bg-surface-2 text-on-surface-tertiary';
  };

  const getDecision = (log: AnalysisLog) => {
    if (log.result?.action) return log.result;
    if (log.result?.decision) return log.result.decision;
    return null;
  };

  if (loading && logs.length === 0) {
    return (
      <div className="flex items-center justify-center py-20 text-on-surface-tertiary text-xs">
        <svg className="animate-spin h-4 w-4 mr-2" viewBox="0 0 24 24" fill="none">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
        </svg>
        加载中...
      </div>
    );
  }

  if (logs.length === 0) {
    return <div className="text-center py-20 text-on-surface-tertiary text-xs">暂无 AI 决策记录</div>;
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
                  <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${getActionColor(decision.action!)}`}>
                    {getActionLabel(decision.action!)}
                  </span>
                )}
                {log.status === 'failed' && (
                  <span className="text-[10px] font-medium px-1.5 py-0.5 rounded bg-red-500/10 text-red-400">失败</span>
                )}
                <span className="text-[11px] text-on-surface-tertiary">
                  {new Date(log.created_at).toLocaleString('zh-CN')}
                </span>
              </div>
              <svg className="w-3.5 h-3.5 text-on-surface-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2">
                <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
              </svg>
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
}
