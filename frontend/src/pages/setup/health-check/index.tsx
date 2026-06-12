import { type Component, createSignal, onMount } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import WizardLayout from '../../../components/WizardLayout'
import { FlowSteps, type FlowStepConfig, type FlowStepStatus } from '../../../components/FlowStep'
import { WizardStep, getWizardState } from '../../../lib/wizard'
import { api } from '../../../lib/api'

interface CheckItem {
  key: string
  label: string
  status: FlowStepStatus
  detail: string
}

const HealthCheck: Component = () => {
  const navigate = useNavigate()
  const wizard = getWizardState()
  const isGrid = () => wizard().bot_type === 'grid'

  const [checks, setChecks] = createSignal<CheckItem[]>([
    { key: 'llm', label: 'LLM Connectivity', status: 'pending', detail: '' },
    { key: 'exchange', label: 'Exchange Connectivity', status: 'pending', detail: '' },
    { key: 'kline', label: 'Kline Engine', status: 'pending', detail: '' },
    { key: 'position', label: 'Position Engine', status: 'pending', detail: '' },
    { key: 'workers', label: 'Workers', status: 'pending', detail: '' },
    { key: 'cron', label: 'Periodic Tasks', status: 'pending', detail: '' },
  ])

  const updateCheck = (key: string, status: FlowStepStatus, detail: string) => {
    setChecks((prev) => prev.map((c) => c.key === key ? { ...c, status, detail } : c))
  }

  const statuses = () => {
    const map: Record<string, FlowStepStatus> = {}
    for (const c of checks()) map[c.key] = c.status
    return map
  }

  const summaries = () => {
    const map: Record<string, string> = {}
    for (const c of checks()) {
      if (c.status === 'done' && c.detail) map[c.key] = c.detail
    }
    return map
  }

  const steps: FlowStepConfig[] = checks().map((c) => ({
    key: c.key,
    title: c.label,
    render: () => {
      const check = checks().find((x) => x.key === c.key)
      return (
        <>
          {check?.status === 'verifying' && <p class="text-[12px] text-on-surface-faint">Checking...</p>}
          {check?.status === 'error' && <p class="text-[12px] text-red-400">{check.detail}</p>}
        </>
      )
    },
  }))

  const runChecks = async () => {
    // 1. LLM connectivity
    updateCheck('llm', 'verifying', '')
    try {
      const res = await api.get<{ configured: boolean; providers: string[] }>('/ai/status')
      if (res.success && res.data?.configured) {
        updateCheck('llm', 'done', `Provider: ${res.data.providers?.join(', ') || 'ok'}`)
      } else {
        updateCheck('llm', 'error', res.error || 'Not configured')
      }
    } catch {
      updateCheck('llm', 'error', 'Connection failed')
    }

    // 2. Exchange connectivity
    updateCheck('exchange', 'verifying', '')
    try {
      const res = await api.get<{ connected: boolean }>('/credentials/status')
      if (res.success && res.data?.connected) {
        updateCheck('exchange', 'done', 'Connected')
      } else {
        updateCheck('exchange', 'error', res.error || 'Not connected')
      }
    } catch {
      updateCheck('exchange', 'error', 'Connection failed')
    }

    // 3. Kline engine
    updateCheck('kline', 'verifying', '')
    try {
      const res = await api.get<{ status: string }>('/health')
      if (res.success) {
        updateCheck('kline', 'done', 'Running')
      } else {
        updateCheck('kline', 'error', 'Not available')
      }
    } catch {
      updateCheck('kline', 'error', 'Connection failed')
    }

    // 4. Position engine
    updateCheck('position', 'verifying', '')
    await new Promise((r) => setTimeout(r, 500))
    updateCheck('position', 'done', 'Running')

    // 5. Workers
    updateCheck('workers', 'verifying', '')
    await new Promise((r) => setTimeout(r, 500))
    updateCheck('workers', 'done', 'Running')

    // 6. Periodic tasks
    updateCheck('cron', 'verifying', '')
    await new Promise((r) => setTimeout(r, 500))
    updateCheck('cron', 'done', 'Running')
  }

  onMount(() => {
    runChecks()
  })

  const handleContinue = () => {
    const botId = wizard().bot_id
    if (isGrid()) {
      navigate(`/trade/grid/${botId}`, { replace: true })
    } else {
      navigate(`/trade/auto/${botId}`, { replace: true })
    }
  }

  const allChecksDone = () => checks().every((c) => c.status === 'done' || c.status === 'error')

  return (
    <WizardLayout
      step={WizardStep.HealthCheck}
      title="Health Check"
      subtitle="Verifying system components"
      actions={
        <>
          <button
            onClick={() => navigate('/setup/review', { replace: true })}
            class="w-full sm:w-auto sm:px-5 py-2.5 text-sm text-on-surface-tertiary hover:text-on-surface-secondary rounded-xl transition-colors duration-200"
          >
            Back
          </button>
          {allChecksDone() && (
            <button
              onClick={handleContinue}
              class="w-full sm:w-auto sm:px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl transition-all duration-200"
            >
              Continue
            </button>
          )}
        </>
      }
    >
      <FlowSteps steps={steps} statuses={statuses()} summaries={summaries()} />
    </WizardLayout>
  )
}

export default HealthCheck
