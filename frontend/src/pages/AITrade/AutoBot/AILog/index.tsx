import { useCallback, useEffect, useState } from 'react';
import { getAutoAnalysisLogs, type AnalysisLog } from '@/service';
import AILogList from '../../components/AILogList';
import { useBot } from '../../context/BotContext';

const PAGE_SIZE = 20;

const AILog = () => {
  const { bot } = useBot();
  const [logs, setLogs] = useState<AnalysisLog[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);

  const loadLogs = useCallback(
    async (p: number) => {
      if (!bot?.id) return;
      setLoading(true);
      try {
        const res = await getAutoAnalysisLogs(bot.id, p, PAGE_SIZE);
        if (res.success && res.data) {
          setLogs(res.data.items || []);
          setTotal(res.data.total || 0);
          setPage(p);
        }
      } catch (e) {
        console.error('Failed to load analysis logs:', e);
      } finally {
        setLoading(false);
      }
    },
    [bot?.id],
  );

  useEffect(() => {
    loadLogs(1);
  }, [loadLogs]);

  if (!bot?.id) return null;

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  return (
    <div className="h-full overflow-y-auto max-w-5xl mx-auto">
      <AILogList logs={logs} loading={loading} botType="auto" botId={bot.id} />

      {/* 分页 */}
      {totalPages > 1 && (
        <div className="flex items-center justify-between px-4 md:px-8 py-3 text-xs text-on-surface-tertiary">
          <span>
            共 {total} 条 · 第 {page}/{totalPages} 页
          </span>
          <div className="flex items-center gap-2">
            <button
              onClick={() => loadLogs(page - 1)}
              disabled={page <= 1 || loading}
              className="px-2 py-1 rounded border border-line-default text-on-surface-secondary disabled:opacity-40 hover:bg-surface-2"
            >
              上一页
            </button>
            <button
              onClick={() => loadLogs(page + 1)}
              disabled={page >= totalPages || loading}
              className="px-2 py-1 rounded border border-line-default text-on-surface-secondary disabled:opacity-40 hover:bg-surface-2"
            >
              下一页
            </button>
          </div>
        </div>
      )}
    </div>
  );
};

export default AILog;
