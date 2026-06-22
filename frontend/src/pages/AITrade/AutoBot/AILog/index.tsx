import { memo, useCallback, useEffect, useState } from 'react'
import { getAutoAnalysisLogs, type AnalysisLog } from '@/service'
import AILogList from '../../components/AILogList'
import { useBot } from '../../context/BotContext'

const AILog = () => {
  const { bot } = useBot()
  const [logs, setLogs] = useState<AnalysisLog[]>([])
  const [loading, setLoading] = useState(false)
  const loadLogs = useCallback(async (botId: string) => {
    setLoading(true)
    try {
      const res = await getAutoAnalysisLogs(botId)
      if (res.data?.logs) setLogs(res.data.logs)
    } catch (e) {
      console.error('Failed to load analysis logs:', e)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (!bot?.id) return
    loadLogs(bot?.id)
  }, [bot?.id, loadLogs])

  if (!bot?.id) return null

  return <AILogList logs={logs} loading={loading} botType="auto" botId={bot?.id} />
}

export default memo(AILog)
