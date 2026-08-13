import { useCallback, useEffect, useRef, useState } from 'react';
import { getAutoAnalysisLogs, type AnalysisLog } from '@/service';
import AILogList from '../../components/LogList';
import { useBot } from '../../context/BotContext';

const PAGE_SIZE = 20;

const Log = () => {
  const { bot } = useBot();
  const [logs, setLogs] = useState<AnalysisLog[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  const loadLogs = useCallback(
    async (p: number, append: boolean) => {
      if (!bot?.id) return;
      setLoading(true);
      try {
        const res = await getAutoAnalysisLogs(bot.id, p, PAGE_SIZE);
        if (res.success && res.data) {
          const items = res.data.items || [];
          setLogs((prev) => (append ? [...prev, ...items] : items));
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
    loadLogs(1, false);
  }, [loadLogs]);

  const hasMore = logs.length < total;

  useEffect(() => {
    const el = sentinelRef.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      (entries) => {
        const entry = entries[0];
        if (entry.isIntersecting && !loading && hasMore) {
          loadLogs(page + 1, true);
        }
      },
      { rootMargin: '200px 0px' },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [loading, hasMore, page, loadLogs]);

  if (!bot?.id) return null;

  return (
    <div className="h-full overflow-y-auto">
      <AILogList logs={logs} loading={loading} botId={bot.id} total={total} />

      <div
        ref={sentinelRef}
        className="h-12 flex items-center justify-center text-xs text-on-surface-tertiary"
      >
        {loading ? '加载中...' : hasMore ? '上拉加载更多' : logs.length > 0 ? '已加载全部' : ''}
      </div>
    </div>
  );
};

export default Log;
