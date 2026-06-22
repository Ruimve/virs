import { useCallback, useState, memo, useEffect } from 'react'
import { getGridAnalysisLogs, type AnalysisLog } from '@/service'
import AILogList from '../../components/AILogList'
import { useBot } from '../../context/BotContext'

const AILog = () => {
  const { bot } = useBot()
  const [logs, setLogs] = useState<AnalysisLog[]>([])
  const [loading, setLoading] = useState(false)

  const loadLogs = useCallback(async (botId: string) => {
    setLoading(true)
    try {
      const res = await getGridAnalysisLogs(botId)
      setLogs(res.data?.items || [])
    } catch (e) {
      console.error(e)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (!bot?.id) return
    loadLogs(bot?.id)
  }, [bot?.id, loadLogs])

  if (!bot?.id) {
    return null
  }
  return <AILogList logs={logs} loading={loading} botType="grid" botId={bot?.id} />
}

export default memo(AILog)
