import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { checkHealth, fetchCredentialStatus, getAiStatus, getUserInfo, findActiveBot } from '../../service'

interface CheckItem {
  name: string
  status: 'pending' | 'checking' | 'ok' | 'warn' | 'error'
  detail: string
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function ServiceCheck() {
  const navigate = useNavigate()
  const [checks, setChecks] = useState<CheckItem[]>([
    { name: 'API Server', status: 'pending', detail: '' },
    { name: 'User Session', status: 'pending', detail: '' },
    { name: 'Exchange Connection', status: 'pending', detail: '' },
    { name: 'AI Service', status: 'pending', detail: '' },
  ])
  const [progress, setProgress] = useState(0)
  const [allDone, setAllDone] = useState(false)
  const [hasError, setHasError] = useState(false)

  function updateCheck(index: number, status: CheckItem['status'], detail: string) {
    setChecks((prev) =>
      prev.map((c, i) => (i === index ? { ...c, status, detail } : c))
    )
  }

  useEffect(() => {
    let cancelled = false
    const total = checks.length
    let completed = 0
    let localHasError = false

    const run = async () => {
      // 1. API Server
      updateCheck(0, 'checking', '')
      try {
        const res = await checkHealth()
        if (res.success) {
          updateCheck(0, 'ok', 'Connected')
        } else {
          updateCheck(0, 'error', res.error || 'Unreachable')
          localHasError = true
        }
      } catch {
        updateCheck(0, 'error', 'Connection refused')
        localHasError = true
      }
      completed++
      if (cancelled) return
      setProgress(Math.round((completed / total) * 100))

      // 2. User Session
      updateCheck(1, 'checking', '')
      try {
        const res = await getUserInfo()
        if (res.success && res.data) {
          updateCheck(1, 'ok', res.data.username)
        } else {
          updateCheck(1, 'error', 'Session expired')
          localHasError = true
        }
      } catch {
        updateCheck(1, 'error', 'Failed')
        localHasError = true
      }
      completed++
      if (cancelled) return
      setProgress(Math.round((completed / total) * 100))

      // 3. Exchange Connection
      updateCheck(2, 'checking', '')
      try {
        const res = await fetchCredentialStatus()
        if (res.success && res.data) {
          if (res.data.connected) {
            updateCheck(2, 'ok', res.data.exchange || 'Connected')
          } else {
            updateCheck(2, 'warn', 'Not configured')
          }
        } else {
          updateCheck(2, 'warn', 'No exchange configured')
        }
      } catch {
        updateCheck(2, 'warn', 'Check skipped')
      }
      completed++
      if (cancelled) return
      setProgress(Math.round((completed / total) * 100))

      // 4. AI Service
      updateCheck(3, 'checking', '')
      try {
        const res = await getAiStatus()
        if (res.success && res.data) {
          if (res.data.configured) {
            updateCheck(3, 'ok', res.data.providers?.join(', ') || 'Configured')
          } else {
            updateCheck(3, 'warn', 'Not configured')
          }
        } else {
          updateCheck(3, 'warn', 'Not configured')
        }
      } catch {
        updateCheck(3, 'warn', 'Check skipped')
      }
      completed++
      if (cancelled) return
      setProgress(100)

      setHasError(localHasError)
      setAllDone(true)

      if (!localHasError) {
        await delay(600)
        if (cancelled) return
        const bot = await findActiveBot()
        if (bot) {
          if (bot.bot_type === 'auto') {
            navigate(`/trade/auto/${bot.id}`, { replace: true })
          } else {
            navigate(`/trade/grid/${bot.id}`, { replace: true })
          }
        } else {
          navigate('/setup/bot-type', { replace: true })
        }
      }
    }

    run()
    return () => { cancelled = true }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  function statusIcon(status: CheckItem['status']) {
    switch (status) {
      case 'ok':
        return <svg className="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2"><path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" /></svg>
      case 'warn':
        return <svg className="w-4 h-4 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2"><path strokeLinecap="round" strokeLinejoin="round" d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
      case 'error':
        return <svg className="w-4 h-4 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2"><path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
      case 'checking':
        return <svg className="w-4 h-4 text-indigo-400 animate-spin" fill="none" viewBox="0 0 24 24"><circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" /><path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" /></svg>
      default:
        return <div className="w-4 h-4 rounded-full border border-line-emphasis" />
    }
  }

  return (
    <div className="min-h-screen bg-base flex flex-col items-center justify-center relative overflow-hidden">
      <div className="absolute inset-0 overflow-hidden">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      <div className="relative mb-12">
        <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-indigo-500/20 to-violet-500/20 border border-indigo-500/20 flex items-center justify-center backdrop-blur-sm">
          <span className="text-2xl font-extralight tracking-[0.3em] text-on-base select-none">V</span>
        </div>
      </div>

      <h1 className="text-xl font-extralight tracking-[0.4em] text-on-surface mb-8 select-none">VIRS</h1>

      <div className="w-80 space-y-3 mb-8">
        {checks.map((check, i) => (
          <div key={i} className="flex items-center gap-3 px-4 py-3 bg-surface-1 border border-line-default rounded-xl">
            <div className="shrink-0">{statusIcon(check.status)}</div>
            <div className="flex-1 min-w-0">
              <div className="text-sm text-on-surface-secondary">{check.name}</div>
              {check.detail && (
                <div className={`text-xs mt-0.5 ${
                  check.status === 'ok' ? 'text-emerald-400/60' :
                  check.status === 'warn' ? 'text-amber-400/60' :
                  check.status === 'error' ? 'text-red-400/60' :
                  'text-on-surface-faint'
                }`}>
                  {check.detail}
                </div>
              )}
            </div>
          </div>
        ))}
      </div>

      <div className="w-64 relative">
        <div className="h-[2px] bg-line-default rounded-full overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-indigo-400 to-violet-400 rounded-full transition-all duration-500 ease-out"
            style={{ width: `${progress}%` }}
          />
        </div>
        <div className="flex justify-between items-center mt-3">
          <span className="text-[11px] text-on-surface-faint tracking-wider">
            {allDone ? (hasError ? 'Some services unavailable' : 'All services ready') : 'Checking services...'}
          </span>
          <span className="text-[11px] text-on-surface-tertiary font-mono tabular-nums">{progress}%</span>
        </div>
      </div>

      {allDone && hasError && (
        <div className="mt-8 flex gap-3">
          <button
            className="px-4 py-2 bg-surface-3 border border-line-strong text-on-surface-tertiary text-sm rounded-xl hover:bg-line-emphasis transition-all"
            onClick={() => window.location.reload()}
          >
            Retry
          </button>
          <button
            className="px-4 py-2 bg-indigo-500/80 text-white text-sm rounded-xl hover:bg-indigo-500 transition-all"
            onClick={async () => {
              const bot = await findActiveBot()
              if (bot) {
                if (bot.bot_type === 'auto') {
                  navigate(`/trade/auto/${bot.id}`, { replace: true })
                } else {
                  navigate(`/trade/grid/${bot.id}`, { replace: true })
                }
              } else {
                navigate('/setup/bot-type', { replace: true })
              }
            }}
          >
            Continue anyway
          </button>
        </div>
      )}
    </div>
  )
}

export default ServiceCheck
