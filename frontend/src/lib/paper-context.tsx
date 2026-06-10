import { createContext, useContext, createSignal, type JSX } from 'solid-js'
import { api } from './api'

interface PaperContextType {
  enabled: () => boolean
  loading: () => boolean
  toggle: () => Promise<void>
  refresh: () => Promise<void>
}

const PaperContext = createContext<PaperContextType>()

export function PaperProvider(props: { children: JSX.Element }) {
  const [enabled, setEnabled] = createSignal(false)
  const [loading, setLoading] = createSignal(false)

  const refresh = async () => {
    try {
      const res = await api.get<{ paper_mode: boolean }>('/system/paper/status')
      if (res.success && res.data) setEnabled(res.data.paper_mode)
    } catch {
      // ignore
    }
  }

  const toggle = async () => {
    if (loading()) return
    setLoading(true)
    try {
      if (enabled()) {
        await api.post('/system/paper/disable')
        setEnabled(false)
      } else {
        await api.post('/system/paper/enable')
        setEnabled(true)
      }
    } catch {
      // ignore
    } finally {
      setLoading(false)
    }
  }

  // init
  refresh()

  return (
    <PaperContext.Provider value={{ enabled, loading, toggle, refresh }}>
      {props.children}
    </PaperContext.Provider>
  )
}

export function usePaper() {
  const ctx = useContext(PaperContext)
  if (!ctx) throw new Error('usePaper must be used within PaperProvider')
  return ctx
}
