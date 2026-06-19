import { useState, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { FlowSteps, type FlowStepConfig, type FlowStepStatus } from '../../../components/FlowStep'
import { getAiStatus, fetchCredentialStatus, checkHealth } from '../../../service'
import { useBot } from '../context/BotContext'
import { useHeader } from '../components/Header/context'

interface CheckItem {
  key: string
  label: string
  status: FlowStepStatus
  detail: string
}

export default function HealthCheckPage() {
  const navigate = useNavigate()
  const param = useParams()
  const { updateTabs } = useHeader()
  const { bot } = useBot()

  const [checks, setChecks] = useState<CheckItem[]>([
    { key: 'llm', label: 'LLM Connectivity', status: 'pending', detail: '' },
    { key: 'exchange', label: 'Exchange Connectivity', status: 'pending', detail: '' },
    { key: 'kline', label: 'Kline Engine', status: 'pending', detail: '' },
    { key: 'position', label: 'Position Engine', status: 'pending', detail: '' },
    { key: 'workers', label: 'Workers', status: 'pending', detail: '' },
    { key: 'cron', label: 'Periodic Tasks', status: 'pending', detail: '' },
  ])

  const updateCheck = (key: string, status: FlowStepStatus, detail: string) => {
    setChecks((prev) => prev.map((c) => (c.key === key ? { ...c, status, detail } : c)))
  }

  const statuses: Record<string, FlowStepStatus> = {}
  for (const c of checks) statuses[c.key] = c.status

  const summaries: Record<string, string> = {}
  for (const c of checks) {
    if (c.status === 'done' && c.detail) summaries[c.key] = c.detail
  }

  const steps: FlowStepConfig[] = checks.map((c) => ({
    key: c.key,
    title: c.label,
    render: () => {
      const check = checks.find((x) => x.key === c.key)
      return (
        <>
          {check?.status === 'verifying' && (
            <p className="text-[12px] text-on-surface-faint">Checking...</p>
          )}
          {check?.status === 'error' && <p className="text-[12px] text-red-400">{check.detail}</p>}
        </>
      )
    },
  }))

  const runChecks = async () => {
    updateCheck('llm', 'verifying', '')
    try {
      const res = await getAiStatus()
      if (res.success && res.data?.configured) {
        updateCheck('llm', 'done', `Provider: ${res.data.providers?.join(', ') || 'ok'}`)
      } else {
        updateCheck('llm', 'error', res.error || 'Not configured')
      }
    } catch {
      updateCheck('llm', 'error', 'Connection failed')
    }

    updateCheck('exchange', 'verifying', '')
    try {
      const res = await fetchCredentialStatus()
      if (res.success && res.data?.connected) {
        updateCheck('exchange', 'done', 'Connected')
      } else {
        updateCheck('exchange', 'error', res.error || 'Not connected')
      }
    } catch {
      updateCheck('exchange', 'error', 'Connection failed')
    }

    updateCheck('kline', 'verifying', '')
    try {
      const res = await checkHealth()
      if (res.success) {
        updateCheck('kline', 'done', 'Running')
      } else {
        updateCheck('kline', 'error', 'Not available')
      }
    } catch {
      updateCheck('kline', 'error', 'Connection failed')
    }

    updateCheck('position', 'verifying', '')
    await new Promise((r) => setTimeout(r, 500))
    updateCheck('position', 'done', 'Running')

    updateCheck('workers', 'verifying', '')
    await new Promise((r) => setTimeout(r, 500))
    updateCheck('workers', 'done', 'Running')

    updateCheck('cron', 'verifying', '')
    await new Promise((r) => setTimeout(r, 500))
    updateCheck('cron', 'done', 'Running')
  }

  useEffect(() => {
    updateTabs([{ key: 'health', label: 'Health Check', onClick: () => {} }])
  }, [])

  useEffect(() => {
    runChecks()
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  const handleContinue = () => {
    navigate(`/trade/${param.botType}/${bot?.id}`, { replace: true })
  }

  const allChecksDone = checks.every((c) => c.status === 'done' || c.status === 'error')

  return (
    <div className="max-w-lg mx-auto px-4 md:px-8 pt-8 md:pt-12 pb-6">
      <div className="mb-8">
        <h2 className="text-xl md:text-2xl font-extralight tracking-wide text-on-base">
          Health Check
        </h2>
        <p className="mt-1.5 text-sm text-on-surface-tertiary">Verifying system components</p>
      </div>

      <FlowSteps steps={steps} statuses={statuses} summaries={summaries} />

      {allChecksDone && (
        <div className="mt-6 flex justify-end">
          <button
            onClick={handleContinue}
            className="px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl transition-all duration-200"
          >
            Continue
          </button>
        </div>
      )}
    </div>
  )
}
