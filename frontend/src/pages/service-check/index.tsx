import { type Component, createSignal, onMount, Show, For } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import { api, getAiStatus, getUserInfo, findActiveBot } from '../../lib/api'

interface CheckItem {
  name: string
  status: 'pending' | 'checking' | 'ok' | 'warn' | 'error'
  detail: string
}

const ServiceCheck: Component = () => {
  const navigate = useNavigate()
  const [checks, setChecks] = createSignal<CheckItem[]>([
    { name: 'API Server', status: 'pending', detail: '' },
    { name: 'User Session', status: 'pending', detail: '' },
    { name: 'Exchange Connection', status: 'pending', detail: '' },
    { name: 'AI Service', status: 'pending', detail: '' },
  ])
  const [progress, setProgress] = createSignal(0)
  const [allDone, setAllDone] = createSignal(false)
  const [hasError, setHasError] = createSignal(false)

  function updateCheck(index: number, status: CheckItem['status'], detail: string) {
    setChecks((prev) =>
      prev.map((c, i) => (i === index ? { ...c, status, detail } : c))
    )
  }

  onMount(async () => {
    const total = checks().length
    let completed = 0

    // 1. API Server
    updateCheck(0, 'checking', '')
    try {
      const res = await api.get('/health')
      if (res.success) {
        updateCheck(0, 'ok', 'Connected')
      } else {
        updateCheck(0, 'error', res.error || 'Unreachable')
        setHasError(true)
      }
    } catch {
      updateCheck(0, 'error', 'Connection refused')
      setHasError(true)
    }
    completed++
    setProgress(Math.round((completed / total) * 100))

    // 2. User Session
    updateCheck(1, 'checking', '')
    try {
      const res = await getUserInfo()
      if (res.success && res.data) {
        updateCheck(1, 'ok', res.data.username)
      } else {
        updateCheck(1, 'error', 'Session expired')
        setHasError(true)
      }
    } catch {
      updateCheck(1, 'error', 'Failed')
      setHasError(true)
    }
    completed++
    setProgress(Math.round((completed / total) * 100))

    // 3. Exchange Connection
    updateCheck(2, 'checking', '')
    try {
      const res = await api.get('/credentials/status')
      if (res.success) {
        const data = res.data as { connected?: boolean; exchange?: string } | null
        if (data?.connected) {
          updateCheck(2, 'ok', data.exchange || 'Connected')
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
    setProgress(Math.round((completed / total) * 100))

    // 4. AI Service
    updateCheck(3, 'checking', '')
    try {
      const res = await getAiStatus()
      if (res.success && res.data) {
        const data = res.data as { configured: boolean; providers: string[] }
        if (data.configured) {
          updateCheck(3, 'ok', data.providers?.join(', ') || 'Configured')
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
    setProgress(100)

    setAllDone(true)

    // Auto-navigate if no critical errors
    if (!hasError()) {
      await delay(600)
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
  })

  function statusIcon(status: CheckItem['status']) {
    switch (status) {
      case 'ok':
        return (
          <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
          </svg>
        )
      case 'warn':
        return (
          <svg class="w-4 h-4 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        )
      case 'error':
        return (
          <svg class="w-4 h-4 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
          </svg>
        )
      case 'checking':
        return (
          <svg class="w-4 h-4 text-indigo-400 animate-spin" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
        )
      default:
        return <div class="w-4 h-4 rounded-full border border-line-emphasis" />
    }
  }

  return (
    <div class="min-h-screen bg-base flex flex-col items-center justify-center relative overflow-hidden">
      {/* Background */}
      <div class="absolute inset-0 overflow-hidden">
        <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      {/* Logo */}
      <div class="relative mb-12">
        <div class="w-16 h-16 rounded-2xl bg-gradient-to-br from-indigo-500/20 to-violet-500/20 border border-indigo-500/20 flex items-center justify-center backdrop-blur-sm">
          <span class="text-2xl font-extralight tracking-[0.3em] text-on-base select-none">V</span>
        </div>
      </div>

      <h1 class="text-xl font-extralight tracking-[0.4em] text-on-surface mb-8 select-none">VIRS</h1>

      {/* Service checks list */}
      <div class="w-80 space-y-3 mb-8">
        <For each={checks()}>
          {(check) => (
            <div class="flex items-center gap-3 px-4 py-3 bg-surface-1 border border-line-default rounded-xl">
              <div class="shrink-0">{statusIcon(check.status)}</div>
              <div class="flex-1 min-w-0">
                <div class="text-sm text-on-surface-secondary">{check.name}</div>
                <Show when={check.detail}>
                  <div class={`text-xs mt-0.5 ${
                    check.status === 'ok' ? 'text-emerald-400/60' :
                    check.status === 'warn' ? 'text-amber-400/60' :
                    check.status === 'error' ? 'text-red-400/60' :
                    'text-on-surface-faint'
                  }`}>
                    {check.detail}
                  </div>
                </Show>
              </div>
            </div>
          )}
        </For>
      </div>

      {/* Progress bar */}
      <div class="w-64 relative">
        <div class="h-[2px] bg-line-default rounded-full overflow-hidden">
          <div
            class="h-full bg-gradient-to-r from-indigo-400 to-violet-400 rounded-full transition-all duration-500 ease-out"
            style={{ width: `${progress()}%` }}
          />
        </div>
        <div class="flex justify-between items-center mt-3">
          <span class="text-[11px] text-on-surface-faint tracking-wider">
            {allDone() ? (hasError() ? 'Some services unavailable' : 'All services ready') : 'Checking services...'}
          </span>
          <span class="text-[11px] text-on-surface-tertiary font-mono tabular-nums">{progress()}%</span>
        </div>
      </div>

      {/* Error actions */}
      <Show when={allDone() && hasError()}>
        <div class="mt-8 flex gap-3">
          <button
            class="px-4 py-2 bg-surface-3 border border-line-strong text-on-surface-tertiary text-sm rounded-xl hover:bg-line-emphasis transition-all"
            onClick={() => window.location.reload()}
          >
            Retry
          </button>
          <button
            class="px-4 py-2 bg-indigo-500/80 text-white text-sm rounded-xl hover:bg-indigo-500 transition-all"
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
      </Show>
    </div>
  )
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

export default ServiceCheck
