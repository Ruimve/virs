import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import {
  getAutoBotDetail,
  getGridBotDetail,
  type AutoBot,
  type AutoTrade,
  type GridBot,
  type GridLevelInfo,
  type GridTrade,
} from '@/service'
import { useLocation, useParams } from 'react-router-dom'

interface BotContextType {
  bot: AutoBot | GridBot | null
  trades: AutoTrade[] | GridTrade[]
  gridLevels: GridLevelInfo[]
  loading: boolean
}

export const BotContext = createContext<BotContextType>({
  bot: null,
  gridLevels: [],
  loading: false,
  trades: [],
})

export const BotProvider = ({ children }: { children: ReactNode }) => {
  const location = useLocation()
  const params = useParams()
  const [bot, setBot] = useState<AutoBot | GridBot | null>(null)
  const [trades, setTrades] = useState<AutoTrade[] | GridTrade[]>([])
  const [gridLevels, setGridLevels] = useState<GridLevelInfo[]>([])
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    const botType = location.pathname.split('/')[2]
    const botId = params?.botId
    if (!botId) return

    if (botType === 'auto') {
      setLoading(true)
      getAutoBotDetail(botId)
        .then((bot) => {
          setBot(bot?.data?.bot || null)
          setTrades(bot?.data?.trades || [])
        })
        .finally(() => {
          setLoading(false)
        })
    } else if (botType === 'grid') {
      setLoading(true)
      getGridBotDetail(botId)
        .then((bot) => {
          setBot(bot?.data?.bot || null)
          setTrades(bot?.data?.trades || [])
          setGridLevels(bot?.data?.grid_levels || [])
        })
        .finally(() => {
          setLoading(false)
        })
    }
  }, [])

  return (
    <BotContext.Provider value={{ bot, trades, gridLevels, loading }}>
      {children}
    </BotContext.Provider>
  )
}

export const useBot = () => {
  const context = useContext(BotContext)
  if (!context) {
    throw new Error('useBot 必须在 BotProvider 内部使用')
  }
  return context
}
