import { useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { getBotAnalysisLogs, type AnalysisLog } from '@/service';
import { useBot } from '@/context/BotContext';
import AILogDetail from '../../../components/LogList/LogDetail';

const Detail = () => {
  const params = useParams();
  const { bot } = useBot();
  const [log, setLog] = useState<AnalysisLog>();
  const [loading, setLoading] = useState(false);

  const loadLog = useCallback(
    async (botId: string) => {
      setLoading(true);
      try {
        const res = await getBotAnalysisLogs({ botId, page: 1, pageSize: 50 });
        const logs = res?.data?.items || [];
        if (logs?.length > 0) {
          const found = logs?.find((l: AnalysisLog) => l.id === params.logId);

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
    loadLog(bot.id);
  }, [bot.id, loadLog]);

  if (!log) return null;

  return (
    <>
      <title>日志详情 - VIRS</title>
      <AILogDetail log={log} loading={loading} />
    </>
  );
};

export default Detail;
