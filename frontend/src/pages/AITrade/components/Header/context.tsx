import { createContext, useContext, useMemo, useState, type ReactNode } from 'react'
import { useLocation } from 'react-router-dom'

export type ItemConfig = {
  key: string
  label: string
  className?: string
  onClick: (key: string) => void
}

export interface HeaderContextType {
  left: ItemConfig[]
  tabs: ItemConfig[]
  actions: ItemConfig[]
  activeTab?: string
  updateLeft: (tabs: ItemConfig[]) => void
  updateTabs: (tabs: ItemConfig[]) => void
  updateActions: (tabs: ItemConfig[]) => void
}

export const DEFAULT_HEADER: HeaderContextType = {
  left: [],
  tabs: [],
  actions: [],
  updateLeft: () => void 0,
  updateTabs: () => void 0,
  updateActions: () => void 0,
}

export const HeaderContext = createContext<HeaderContextType>(DEFAULT_HEADER)

export const HeaderProvider = ({ children }: { children: ReactNode }) => {
  const location = useLocation()
  const [left, setLeft] = useState<ItemConfig[]>([])
  const [tabs, setTabs] = useState<ItemConfig[]>([])
  const [actions, setActions] = useState<ItemConfig[]>([])

  const activeTab = useMemo(() => {
    const paths = location.pathname.split('/') || []
    return paths[paths.length - 1]
  }, [location.pathname])

  return (
    <HeaderContext.Provider
      value={{
        left,
        tabs,
        actions,
        activeTab,
        updateLeft: setLeft,
        updateTabs: setTabs,
        updateActions: setActions,
      }}
    >
      {children}
    </HeaderContext.Provider>
  )
}

export const useHeader = () => {
  const context = useContext(HeaderContext)
  if (!context) {
    throw new Error('useHeader 必须在 HeaderProvider 内部使用')
  }
  return context
}
