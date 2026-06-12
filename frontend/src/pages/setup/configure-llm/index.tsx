import { type Component, createSignal, Show, For, onMount } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import WizardLayout from '../../../components/WizardLayout'
import { FlowSteps, type FlowStepConfig, type FlowStepStatus } from '../../../components/FlowStep'
import { updateWizard, advanceStep, WizardStep, getWizardState } from '../../../lib/wizard'
import { api, saveAiCredential } from '../../../lib/api'

interface DeepSeekModel {
  id: string
  owned_by: string
}

interface BalanceInfo {
  total_balance: string
  currency: string
}

const ConfigureLlm: Component = () => {
  const navigate = useNavigate()
  const wizard = getWizardState()

  // Step 1: API Key + Model
  // API key is no longer held in memory (wizardCredentials). It's saved to backend DB.
  // On mount, we check if backend already has a saved credential.
  const [apiKey, setApiKey] = createSignal('')
  const [model, setModel] = createSignal(wizard().llm_model || '')
  const [models, setModels] = createSignal<DeepSeekModel[]>([])
  const [fetchingModels, setFetchingModels] = createSignal(false)
  const [step1Status, setStep1Status] = createSignal<FlowStepStatus>('active')
  const [error, setError] = createSignal('')

  // Step 2: Connectivity
  const [step2Status, setStep2Status] = createSignal<FlowStepStatus>('pending')

  // Step 3: Account Info (balance)
  const [balance, setBalance] = createSignal<BalanceInfo | null>(null)
  const [step3Status, setStep3Status] = createSignal<FlowStepStatus>('pending')

  const statuses = () => ({
    apiKey: step1Status(),
    connectivity: step2Status(),
    account: step3Status(),
  })

  const summaries = () => ({
    apiKey: step1Status() === 'done' ? `${apiKey().slice(0, 6)}...${apiKey().slice(-4)} · ${model()}` : undefined,
    connectivity: step2Status() === 'done' ? 'Connected to DeepSeek API' : step2Status() === 'error' ? 'Connection failed' : undefined,
    account: step3Status() === 'done' && balance() ? `Balance: ${balance()!.total_balance} ${balance()!.currency}` : undefined,
  })

  const steps: FlowStepConfig[] = [
    {
      key: 'apiKey',
      title: 'API Key',
      editable: true,
      render: () => (
        <div class="space-y-3">
          <div class="relative">
            <input
              type="password"
              value={apiKey()}
              onInput={(e) => onKeyInput(e.currentTarget.value)}
              disabled={fetchingModels()}
              class={`w-full px-4 py-2.5 bg-surface-2 border rounded-lg text-sm text-on-base placeholder-placeholder focus:outline-none transition-all duration-200 ${
                fetchingModels() ? 'border-indigo-500/30 opacity-60' : 'border-line-strong focus:border-indigo-500/40'
              }`}
              placeholder="sk-..."
            />
            <Show when={fetchingModels()}>
              <div class="absolute right-3 top-1/2 -translate-y-1/2">
                <svg class="animate-spin w-4 h-4 text-indigo-400" fill="none" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                </svg>
              </div>
            </Show>
          </div>
          <Show when={error()}>
            <p class="text-[12px] text-red-400">{error()}</p>
          </Show>
          <Show when={models().length > 0}>
            <div>
              <p class="text-[11px] tracking-[0.15em] text-on-surface-muted uppercase mb-2">Model</p>
              <div class="flex flex-wrap gap-1.5">
                <For each={models()}>
                  {(m) => {
                    const isSelected = () => model() === m.id
                    return (
                      <button
                        onClick={() => setModel(m.id)}
                        class={`px-2.5 py-1 rounded-md text-[11px] border transition-all duration-200 ${
                          isSelected()
                            ? 'bg-indigo-500/15 border-indigo-500/30 text-on-surface'
                            : 'bg-surface-1 border-line-default text-on-surface-tertiary hover:bg-surface-2'
                        }`}
                      >
                        {m.id}
                      </button>
                    )
                  }}
                </For>
              </div>
            </div>
          </Show>
          <button
            onClick={verifyApiKey}
            disabled={!apiKey().trim() || !model().trim() || step1Status() === 'verifying' || fetchingModels()}
            class="px-4 py-2 text-[12px] bg-indigo-500/20 border border-indigo-500/30 rounded-lg text-indigo-300 hover:bg-indigo-500/30 disabled:opacity-30 transition-all duration-200"
          >
            <Show when={step1Status() !== 'verifying'} fallback="Verifying...">
              Verify
            </Show>
          </button>
        </div>
      ),
    },
    {
      key: 'connectivity',
      title: 'Connectivity',
      description: 'Test LLM API inference capability',
      render: () => (
        <div class="space-y-2">
          <Show when={step2Status() === 'verifying'}>
            <p class="text-[12px] text-on-surface-tertiary">Testing connection to DeepSeek API...</p>
          </Show>
          <Show when={step2Status() === 'error'}>
            <p class="text-[12px] text-red-400">{error() || 'Connection failed'}</p>
          </Show>
        </div>
      ),
    },
    {
      key: 'account',
      title: 'Account Info',
      render: () => (
        <div class="space-y-3">
          <Show when={balance()}>
            <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
              <span class="text-[12px] text-on-surface-tertiary">Balance</span>
              <span class="text-[12px] text-on-surface-secondary font-mono">{balance()!.total_balance} {balance()!.currency}</span>
            </div>
          </Show>
          <Show when={!balance() && step3Status() === 'active'}>
            <p class="text-[12px] text-on-surface-faint">No balance info available. Ensure your account has credits.</p>
          </Show>
        </div>
      ),
    },
  ]

  // Fetch models via backend proxy (after save)
  const fetchModels = async () => {
    setFetchingModels(true)
    try {
      const result = await api.get<{ models: DeepSeekModel[] }>('/ai-credentials/models')
      if (result.success && result.data?.models) {
        const list = result.data.models
        setModels(list)
        if (list.length > 0) {
          const current = model()
          if (!list.some((m) => m.id === current)) {
            setModel(list[0].id)
          }
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
  }

  // Test connectivity via backend (uses saved credentials)
  const testConnectivity = async () => {
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
  }

  // Fetch balance via backend proxy (uses saved credentials)
  const fetchBalance = async () => {
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
  }

  onMount(async () => {
    // Check if backend already has a saved AI credential.
    // If so, fetch models to populate the UI (user may be returning to this step).
    try {
      const result = await api.get<{ items: Array<{ provider: string; is_default: boolean }> }>('/ai-credentials/list')
      if (result.success && result.data?.items?.length) {
        // Has saved credential — fetch models via backend proxy
        fetchModels()
      }
    } catch {
      // ignore — user will enter key manually
    }
  })

  let debounceTimer: ReturnType<typeof setTimeout> | null = null
  const onKeyInput = (key: string) => {
    setApiKey(key)
    setError('')
    setModels([])
    setModel('')
    if (step1Status() === 'error') setStep1Status('active')
    if (step2Status() !== 'pending') setStep2Status('pending')
    if (step3Status() !== 'pending') setStep3Status('pending')
    // Debounce: save then fetch models
    if (debounceTimer) clearTimeout(debounceTimer)
    if (!key.trim()) return
    debounceTimer = setTimeout(async () => {
      // Save first, then fetch models via backend
      const saveResult = await saveAiCredential({
        provider: 'deepseek',
        api_key: key.trim(),
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
    const key = apiKey().trim()
    if (!key || !model().trim()) return
    setStep1Status('verifying')
    setError('')

    // Save credential to backend first
    const saveResult = await saveAiCredential({
      provider: 'deepseek',
      api_key: key,
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
      llm_model: model(),
    })
    // No longer store llm_api_key in memory — it's already saved to backend DB.
    // All subsequent LLM operations use the saved credential via backend proxy.
    advanceStep(WizardStep.SelectExchange)
    navigate('/setup/exchange', { replace: true })
  }

  const canContinue = () => step2Status() === 'done' && step3Status() === 'done'

  return (
    <WizardLayout
      step={WizardStep.ConfigureLlm}
      title="Configure DeepSeek"
      subtitle="Connect your DeepSeek account"
      actions={
        <>
          <button
            onClick={() => navigate('/setup/bot-type', { replace: true })}
            class="w-full sm:w-auto sm:px-5 py-2.5 text-sm text-on-surface-tertiary hover:text-on-surface-secondary rounded-xl transition-colors duration-200"
          >
            Back
          </button>
          <button
            onClick={handleContinue}
            disabled={!canContinue()}
            class="w-full sm:w-auto sm:px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
          >
            Continue
          </button>
        </>
      }
    >
      <FlowSteps steps={steps} statuses={statuses()} summaries={summaries()} />
    </WizardLayout>
  )
}

export default ConfigureLlm
