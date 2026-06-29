import { useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { getAutoAnalysisLogs, type AnalysisLog } from '@/service';
import AILogDetail from '../../../components/AILogList/AILogDetail';
import { useBot } from '../../../context/BotContext';

const Detail = () => {
  const params = useParams();
  const { bot } = useBot();
  const [log, setLog] = useState<AnalysisLog>();
  const [loading, setLoading] = useState(false);

  const loadLog = useCallback(
    async (botId: string) => {
      setLoading(true);
      try {
        const res = await getAutoAnalysisLogs(botId, 1, 50);
        const logs = res?.data?.items || [];
        if (logs?.length > 0) {
          const found = logs?.find((l: AnalysisLog) => l.id === params.logId);
          // 找不到目标 log 时，回退到最新一条（page_size=1 时即返回最新 log）
          setLog(found ?? logs[0]);
        }
      } catch (e) {
        console.error('Failed to load analysis logs:', e);
      } finally {
        setLoading(false);
      }
    },
    [params.logId],
  );

  useEffect(() => {
    if (!bot?.id) return;
    loadLog(bot?.id);
  }, [bot?.id, loadLog]);

  if (!bot?.id || !log) return null;

  return <AILogDetail log={log} loading={loading} />;
};

export default Detail;
