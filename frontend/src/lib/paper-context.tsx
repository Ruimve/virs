import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react'
import { getPaperStatus, enablePaperMode, disablePaperMode } from '../service'

interface PaperContextType {
  enabled: boolean
  loading: boolean
  toggle: () => Promise<void>
  refresh: () => Promise<void>
}

const PaperContext = createContext<PaperContextType | null>(null)

export function PaperProvider({ children }: { children: ReactNode }) {
  const [enabled, setEnabled] = useState(false)
  const [loading, setLoading] = useState(false)

  const refresh = useCallback(async () => {
    try {
      const res = await getPaperStatus()
      if (res.success && res.data) setEnabled(res.data.paper_mode)
    } catch {
      // ignore
    }
  }, [])

  const toggle = useCallback(async () => {
    if (loading) return
    setLoading(true)
    try {
      if (enabled) {
        await disablePaperMode()
        setEnabled(false)
      } else {
        await enablePaperMode()
        setEnabled(true)
      }
    } catch {
      // ignore
    } finally {
      setLoading(false)
    }
  }, [enabled, loading])

  useEffect(() => {
    refresh()
  }, [refresh])

  return (
    <PaperContext.Provider value={{ enabled, loading, toggle, refresh }}>
      {children}
    </PaperContext.Provider>
  )
}

export function usePaper() {
  const ctx = useContext(PaperContext)
  if (!ctx) throw new Error('usePaper must be used within PaperProvider')
  return ctx
}
