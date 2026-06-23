import { usePositionWs, type PositionWsEvent } from '@/service/ws'
import { createContext, useContext, useState, type ReactNode } from 'react'
import { useBot } from '../BotContext'

interface PositionContextType {
  position: PositionWsEvent | null
}

export const PositionContext = createContext<PositionContextType>({
  position: null,
})

export const PositionProvider = ({ children }: { children: ReactNode }) => {
  const { bot } = useBot()
  const [position, setPosition] = useState<PositionWsEvent | null>(null)

  usePositionWs(bot?.symbol, (event) => {
    if (event.symbol !== bot?.symbol) return
    // 仓位关闭后清空 state，避免残留 closed 数据
    if (event.status === 'closed' || event.size <= 0) {
      setPosition(null)
      return
    }
    setPosition(event)
  })

  return <PositionContext.Provider value={{ position }}>{children}</PositionContext.Provider>
}

export const usePositionContext = () => {
  const context = useContext(PositionContext)
  if (!context) {
    throw new Error('usePositionContext 必须在 PositionProvider 内部使用')
  }
  return context
}
