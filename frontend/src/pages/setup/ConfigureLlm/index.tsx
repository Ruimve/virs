import { useState, useEffect, useCallback, useRef, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { Wizard } from '../components/Wizard';
import { FlowSteps, type FlowStepConfig, type FlowStepStatus } from '../../../components/FlowStep'
import { updateWizard, advanceStep, WizardStep, getWizardState } from '../components/Wizard/wizard'
import { api, saveAiCredential } from '../../../service'

interface DeepSeekModel {
  id: string
  owned_by: string
}

interface BalanceInfo {
  total_balance: string
  currency: string
}

function ConfigureLlm() {
  const navigate = useNavigate()
  const wizard = getWizardState()

  // Step 1: API Key + Model
  const [apiKey, setApiKey] = useState('')
  const [model, setModel] = useState(wizard.llm_model || '')
  const [models, setModels] = useState<DeepSeekModel[]>([])
  const [fetchingModels, setFetchingModels] = useState(false)
  const [step1Status, setStep1Status] = useState<FlowStepStatus>('active')
  const [error, setError] = useState('')

  // Step 2: Connectivity
  const [step2Status, setStep2Status] = useState<FlowStepStatus>('pending')

  // Step 3: Account Info (balance)
  const [balance, setBalance] = useState<BalanceInfo | null>(null)
  const [step3Status, setStep3Status] = useState<FlowStepStatus>('pending')

  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const statuses = {
    apiKey: step1Status,
    connectivity: step2Status,
    account: step3Status,
  }

  const summaries: Record<string, string | ReactNode> = {}
  if (step1Status === 'done') summaries.apiKey = `${apiKey.slice(0, 6)}...${apiKey.slice(-4)} · ${model}`
  if (step2Status === 'done') summaries.connectivity = 'Connected to DeepSeek API'
  else if (step2Status === 'error') summaries.connectivity = 'Connection failed'
  if (step3Status === 'done' && balance) summaries.account = `Balance: ${balance.total_balance} ${balance.currency}`

  // Fetch models via backend proxy (after save)
  const fetchModels = useCallback(async () => {
    setFetchingModels(true)
    try {
      const result = await api.get<{ models: DeepSeekModel[] }>('/ai-credentials/models')
      if (result.success && result.data?.models) {
        const list = result.data.models
        setModels(list)
        if (list.length > 0) {
          setModel(prev => {
            if (!list.some((m) => m.id === prev)) {
              return list[0].id
            }
            return prev
          })
        }
      } else {
        setError(result.error || 'Failed to fetch models')
        setStep1Status('error')
      }
    } catch {
      setError('Network error')
      setStep1Status('error')
    } finally {
      setFetchingModels(false)
    }
  }, [])

  // Test connectivity via backend (uses saved credentials)
  const testConnectivity = useCallback(async () => {
    setStep2Status('verifying')
    try {
      const result = await api.get<{ connected: boolean; message: string }>('/ai-credentials/test')
      if (!result.success || !result.data?.connected) {
        setError(result.data?.message || result.error || 'Connection failed')
        setStep2Status('error')
        return
      }
      setStep2Status('done')
      fetchBalance()
    } catch {
      setError('Network error')
      setStep2Status('error')
    }
  }, [])

  // Fetch balance via backend proxy (uses saved credentials)
  const fetchBalance = useCallback(async () => {
    setStep3Status('verifying')
    try {
      const result = await api.get<{ balances: BalanceInfo[] }>('/ai-credentials/balance')
      if (result.success && result.data?.balances && result.data.balances.length > 0) {
        const bal = result.data.balances[0]
        setBalance(bal)
        const total = parseFloat(bal.total_balance || '0')
        setStep3Status(total > 0 ? 'done' : 'active')
      } else {
        setStep3Status('active')
      }
    } catch {
      setStep3Status('active')
    }
  }, [])

  useEffect(() => {
    // Check if backend already has a saved AI credential.
    const checkSaved = async () => {
      try {
        const result = await api.get<{ items: Array<{ provider: string; is_default: boolean }> }>('/ai-credentials/list')
        if (result.success && result.data?.items?.length) {
          fetchModels()
        }
      } catch {
        // ignore — user will enter key manually
      }
    }
    checkSaved()
  }, [fetchModels])

  const onKeyInput = (key: string) => {
    setApiKey(key)
    setError('')
    setModels([])
    setModel('')
    if (step1Status === 'error') setStep1Status('active')
    if (step2Status !== 'pending') setStep2Status('pending')
    if (step3Status !== 'pending') setStep3Status('pending')
    // Debounce: save then fetch models
    if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current)
    if (!key.trim()) return
    debounceTimerRef.current = setTimeout(async () => {
      const saveResult = await saveAiCredential({
        provider: 'deepseek',
        api_key: key.trim(),
        model: model || undefined,
        is_default: true,
      })
      if (saveResult.success) {
        fetchModels()
      } else {
        setError(saveResult.error || 'Failed to save API key')
        setStep1Status('error')
      }
    }, 600)
  }

  const verifyApiKey = async () => {
    const key = apiKey.trim()
    if (!key || !model.trim()) return
    setStep1Status('verifying')
    setError('')

    // Save credential to backend first
    const saveResult = await saveAiCredential({
      provider: 'deepseek',
      api_key: key,
      model: model,
      is_default: true,
    })
    if (!saveResult.success) {
      setError(saveResult.error || 'Failed to save API key')
      setStep1Status('error')
      return
    }

    setStep1Status('done')
    testConnectivity()
  }

  const handleContinue = () => {
    updateWizard({
      llm_provider: 'deepseek',
      llm_model: model,
    })
    advanceStep(WizardStep.SelectExchange)
    navigate('/setup/exchange', { replace: true })
  }

  const canContinue = step2Status === 'done' && step3Status === 'done'

  const steps: FlowStepConfig[] = [
    {
      key: 'apiKey',
      title: 'API Key',
      editable: true,
      render: () => (
        <div className="space-y-3">
          <div className="relative">
            <input
              type="password"
              value={apiKey}
              onInput={(e) => onKeyInput(e.currentTarget.value)}
              disabled={fetchingModels}
              className={`w-full px-4 py-2.5 bg-surface-2 border rounded-lg text-sm text-on-base placeholder-placeholder focus:outline-none transition-all duration-200 ${
                fetchingModels ? 'border-indigo-500/30 opacity-60' : 'border-line-strong focus:border-indigo-500/40'
              }`}
              placeholder="sk-..."
            />
            {fetchingModels && (
              <div className="absolute right-3 top-1/2 -translate-y-1/2">
                <svg className="animate-spin w-4 h-4 text-indigo-400" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                </svg>
              </div>
            )}
          </div>
          {error && <p className="text-[12px] text-red-400">{error}</p>}
          {models.length > 0 && (
            <div>
              <p className="text-[11px] tracking-[0.15em] text-on-surface-muted uppercase mb-2">Model</p>
              <div className="flex flex-wrap gap-1.5">
                {models.map((m) => (
                  <button
                    key={m.id}
                    onClick={() => setModel(m.id)}
                    className={`px-2.5 py-1 rounded-md text-[11px] border transition-all duration-200 ${
                      model === m.id
                        ? 'bg-indigo-500/15 border-indigo-500/30 text-on-surface'
                        : 'bg-surface-1 border-line-default text-on-surface-tertiary hover:bg-surface-2'
                    }`}
                  >
                    {m.id}
                  </button>
                ))}
              </div>
            </div>
          )}
          <button
            onClick={verifyApiKey}
            disabled={!apiKey.trim() || !model.trim() || step1Status === 'verifying' || fetchingModels}
            className="px-4 py-2 text-[12px] bg-indigo-500/20 border border-indigo-500/30 rounded-lg text-indigo-300 hover:bg-indigo-500/30 disabled:opacity-30 transition-all duration-200"
          >
            {step1Status === 'verifying' ? 'Verifying...' : 'Verify'}
          </button>
        </div>
      ),
    },
    {
      key: 'connectivity',
      title: 'Connectivity',
      description: 'Test LLM API inference capability',
      render: () => (
        <div className="space-y-2">
          {step2Status === 'verifying' && (
            <p className="text-[12px] text-on-surface-tertiary">Testing connection to DeepSeek API...</p>
          )}
          {step2Status === 'error' && (
            <p className="text-[12px] text-red-400">{error || 'Connection failed'}</p>
          )}
        </div>
      ),
    },
    {
      key: 'account',
      title: 'Account Info',
      render: () => (
        <div className="space-y-3">
          {balance && (
            <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
              <span className="text-[12px] text-on-surface-tertiary">Balance</span>
              <span className="text-[12px] text-on-surface-secondary font-mono">{balance.total_balance} {balance.currency}</span>
            </div>
          )}
          {!balance && step3Status === 'active' && (
            <p className="text-[12px] text-on-surface-faint">No balance info available. Ensure your account has credits.</p>
          )}
        </div>
      ),
    },
  ]

  return (
    <Wizard
      step={WizardStep.ConfigureLlm}
      title="Configure DeepSeek"
      subtitle="Connect your DeepSeek account"
      actions={
        <>
          <button
            onClick={() => navigate('/setup/bot-type', { replace: true })}
            className="w-full sm:w-auto sm:px-5 py-2.5 text-sm text-on-surface-tertiary hover:text-on-surface-secondary rounded-xl transition-colors duration-200"
          >
            Back
          </button>
          <button
            onClick={handleContinue}
            disabled={!canContinue}
            className="w-full sm:w-auto sm:px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
          >
            Continue
          </button>
        </>
      }
    >
      <FlowSteps steps={steps} statuses={statuses} summaries={summaries} />
    </Wizard>
  )
}

export default ConfigureLlm
