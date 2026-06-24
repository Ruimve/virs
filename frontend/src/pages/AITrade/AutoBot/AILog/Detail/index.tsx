import { memo, useCallback, useEffect, useState } from 'react';
import { getAutoAnalysisLogs, type AnalysisLog } from '@/service';
import AILogDetail from '../../../components/AILogList/AILogDetail';
import { useBot } from '../../../context/BotContext';
import { useParams } from 'react-router-dom';

const Detail = () => {
  const params = useParams();
  const { bot } = useBot();
  const [log, setLog] = useState<AnalysisLog>();
  const [loading, setLoading] = useState(false);

  const loadLog = useCallback(
    async (botId: string) => {
      setLoading(true);
      try {
        const res = await getAutoAnalysisLogs(botId);
        const logs = res?.data?.logs || [];
        if (logs?.length > 0) {
          const found = logs?.find((l: AnalysisLog) => l.id === params.logId);
          setLog(found);
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

export default memo(Detail);
