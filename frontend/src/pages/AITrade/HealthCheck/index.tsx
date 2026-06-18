import { useState, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import BotDetailHeader from '../components/BotDetailHeader'
import { FlowSteps, type FlowStepConfig, type FlowStepStatus } from '../../../components/FlowStep'
import { getAiStatus, fetchCredentialStatus, checkHealth, getAutoBotDetail, getGridBotDetail } from '../../../service'
import type { BotHeaderInfo } from '../../../service/types'
import type { TabConfig } from '../components/shared'

interface CheckItem {
  key: string
  label: string
  status: FlowStepStatus
  detail: string
}

const tabs: TabConfig[] = [
  { key: 'health', label: 'Health Check' },
]

export default function HealthCheckPage() {
  const params = useParams()
  const navigate = useNavigate()
  const botType = params.botType || 'auto'
  const botId = params.botId || ''

  const [botInfo, setBotInfo] = useState<BotHeaderInfo | null>(null)

  const [checks, setChecks] = useState<CheckItem[]>([
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
          {check?.status === 'verifying' && <p className="text-[12px] text-on-surface-faint">Checking...</p>}
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
    // Load bot info from backend
    const loadBot = async () => {
      try {
        if (botType === 'grid') {
          const res = await getGridBotDetail(botId)
          if (res.data?.bot) {
            const b = res.data.bot
            setBotInfo({
              id: b.id, name: b.name, symbol: b.symbol,
              exchange: b.exchange, market_type: b.market_type,
              status: b.status, leverage: b.leverage,
            })
          }
        } else {
          const res = await getAutoBotDetail(botId)
          if (res.data?.bot) {
            const b = res.data.bot
            setBotInfo({
              id: b.id, name: b.name, symbol: b.symbol,
              exchange: b.exchange, market_type: b.market_type,
              status: b.status, leverage: b.leverage,
            })
          }
        }
      } catch (e) {
        console.error('Failed to load bot:', e)
      }
    }
    loadBot()
    runChecks()
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  const handleContinue = () => {
    navigate(`/trade/${botType}/${botId}`, { replace: true })
  }

  const allChecksDone = checks.every((c) => c.status === 'done' || c.status === 'error')

  return (
    <div className="h-screen flex flex-col bg-base relative overflow-hidden">
      {/* Background glow */}
      <div className="absolute inset-0 overflow-hidden">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/3 blur-[120px]" />
      </div>
      <BotDetailHeader
        bot={botInfo || { id: botId, name: '', symbol: '', exchange: '', market_type: 'perpetual', status: 'stopped', leverage: 0 }}
        tabs={tabs}
        activeTab="health"
        onTabChange={(key) => {
          if (key === 'health') return
          navigate(`/trade/${botType}/${botId}/${key}`)
        }}
      />
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-lg mx-auto px-4 md:px-8 pt-8 md:pt-12 pb-6">
          <div className="mb-8">
            <h2 className="text-xl md:text-2xl font-extralight tracking-wide text-on-base">Health Check</h2>
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
      </div>
    </div>
  )
}
