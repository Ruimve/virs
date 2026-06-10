import { type Component, createSignal, Show, For } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import WizardLayout from '../../../components/WizardLayout'
import { FlowSteps, type FlowStepConfig, type FlowStepStatus } from '../../../components/FlowStep'
import { updateWizard, advanceStep, WizardStep } from '../../../lib/wizard'
import { saveCredential, testCredential, checkPermissions, fetchAccountInfo } from '../../../lib/api'
import type { PermissionItem, AccountInfo } from '../../../lib/api'

const SelectExchange: Component = () => {
  const navigate = useNavigate()

  // Step 1: API credentials
  // Credentials are no longer held in memory (wizardCredentials). They're saved to backend DB.
  // On mount, we check if backend already has saved credentials.
  const [apiKey, setApiKey] = createSignal('')
  const [apiSecret, setApiSecret] = createSignal('')
  const [step1Status, setStep1Status] = createSignal<FlowStepStatus>('active')
  const [error, setError] = createSignal('')

  // Step 2: Connectivity + Permissions (merged — testCredential returns both)
  const [step2Status, setStep2Status] = createSignal<FlowStepStatus>('pending')

  // Step 3: Permissions (via verify — uses saved credentials)
  const [permissions, setPermissions] = createSignal<PermissionItem[]>([])
  const [step3Status, setStep3Status] = createSignal<FlowStepStatus>('pending')

  // Step 4: Account Info
  const [accountInfo, setAccountInfo] = createSignal<AccountInfo | null>(null)
  const [step4Status, setStep4Status] = createSignal<FlowStepStatus>('pending')

  const statuses = () => ({
    credentials: step1Status(),
    connectivity: step2Status(),
    permissions: step3Status(),
    account: step4Status(),
  })

  const summaries = () => ({
    credentials: step1Status() === 'done' ? `${apiKey().slice(0, 6)}...${apiKey().slice(-4)}` : undefined,
    connectivity: step2Status() === 'done' ? 'Connected to Binance' : step2Status() === 'error' ? 'Connection failed' : undefined,
    permissions: step3Status() === 'done' ? 'All checks passed' : undefined,
    account: step4Status() === 'done' ? `Perpetual: ${accountInfo()?.perpetual_usdt?.toFixed(2) ?? '—'} USDT` : undefined,
  })

  const statusIcon = (status: string) => {
    if (status === 'ok') return <span class="text-emerald-400">&#10003;</span>
    if (status === 'warn') return <span class="text-amber-400">&#9888;</span>
    return <span class="text-red-400">&#10007;</span>
  }

  const resetDownstream = () => {
    if (step2Status() !== 'pending') setStep2Status('pending')
    if (step3Status() !== 'pending') setStep3Status('pending')
    if (step4Status() !== 'pending') setStep4Status('pending')
  }

  const steps: FlowStepConfig[] = [
    {
      key: 'credentials',
      title: 'API Credentials',
      render: () => (
        <div class="space-y-3">
          <input
            type="text"
            value={apiKey()}
            onInput={(e) => {
              setApiKey(e.currentTarget.value)
              setError('')
              if (step1Status() === 'error') setStep1Status('active')
              resetDownstream()
            }}
            class="w-full px-4 py-2.5 bg-white/[0.04] border border-white/[0.08] rounded-lg text-sm text-white/90 placeholder-white/20 focus:outline-none focus:border-indigo-500/40 transition-all duration-200"
            placeholder="API Key"
          />
          <input
            type="password"
            value={apiSecret()}
            onInput={(e) => {
              setApiSecret(e.currentTarget.value)
              setError('')
              if (step1Status() === 'error') setStep1Status('active')
              resetDownstream()
            }}
            class="w-full px-4 py-2.5 bg-white/[0.04] border border-white/[0.08] rounded-lg text-sm text-white/90 placeholder-white/20 focus:outline-none focus:border-indigo-500/40 transition-all duration-200"
            placeholder="API Secret"
          />
          <Show when={error()}>
            <p class="text-[12px] text-red-400">{error()}</p>
          </Show>
          <button
            onClick={verifyCredentials}
            disabled={!apiKey().trim() || !apiSecret().trim() || step1Status() === 'verifying'}
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
      description: 'Ping exchange server to verify reachability',
      render: () => (
        <div class="space-y-2">
          <Show when={step2Status() === 'verifying'}>
            <p class="text-[12px] text-white/30">Testing connection to Binance...</p>
          </Show>
          <Show when={step2Status() === 'error'}>
            <p class="text-[12px] text-red-400">{error() || 'Connection failed'}</p>
          </Show>
        </div>
      ),
    },
    {
      key: 'permissions',
      title: 'Permissions',
      description: 'Check API key permissions and restrictions',
      render: () => (
        <div class="space-y-1.5">
          <For each={permissions()}>
            {(p) => (
              <div class="flex items-center justify-between px-3 py-1.5 bg-white/[0.02] border border-white/[0.06] rounded-lg">
                <div class="flex items-center gap-2">
                  <span class="text-[12px]">{statusIcon(p.status)}</span>
                  <span class="text-[12px] text-white/50">{p.label}</span>
                </div>
                <span class={`text-[11px] ${
                  p.status === 'ok' ? 'text-white/25' :
                  p.status === 'warn' ? 'text-amber-400/60' :
                  'text-red-400/60'
                }`}>
                  {p.detail}
                </span>
              </div>
            )}
          </For>
        </div>
      ),
    },
    {
      key: 'account',
      title: 'Account Info',
      description: 'USDT balances across accounts',
      render: () => (
        <div class="space-y-1.5">
          <Show when={step4Status() === 'verifying'}>
            <p class="text-[12px] text-white/30">Fetching account balances...</p>
          </Show>
          <Show when={accountInfo()}>
            <div class="flex items-center justify-between px-3 py-2 bg-white/[0.02] border border-white/[0.06] rounded-lg">
              <span class="text-[12px] text-white/40">Perpetual (USDT)</span>
              <span class="text-[12px] text-white/70 font-mono">{accountInfo()!.perpetual_usdt != null ? accountInfo()!.perpetual_usdt!.toFixed(4) : '—'}</span>
            </div>
            <div class="flex items-center justify-between px-3 py-2 bg-white/[0.02] border border-white/[0.06] rounded-lg">
              <span class="text-[12px] text-white/40">Spot (USDT)</span>
              <span class="text-[12px] text-white/70 font-mono">{accountInfo()!.spot_usdt != null ? accountInfo()!.spot_usdt!.toFixed(4) : '—'}</span>
            </div>
          </Show>
          <Show when={step4Status() === 'error'}>
            <p class="text-[12px] text-red-400">{error() || 'Failed to fetch balances'}</p>
          </Show>
        </div>
      ),
    },
  ]

  // Test connectivity only (ping) — uses saved credentials from registry
  const doTestConnectivity = async () => {
    setStep2Status('verifying')
    try {
      const result = await testCredential()
      if (!result.success || !result.data?.connected) {
        setError(result.data?.message || result.error || 'Connection failed')
        setStep2Status('error')
        return
      }
      setStep2Status('done')
      // Auto-advance to permissions check
      doCheckPermissions()
    } catch {
      setError('Connection test failed')
      setStep2Status('error')
    }
  }

  // Check permissions via apiRestrictions — uses saved credentials from registry
  const doCheckPermissions = async () => {
    setStep3Status('verifying')
    try {
      const result = await checkPermissions()
      if (!result.success || !result.data?.permissions) {
        setError(result.error || 'Permission check failed')
        setStep3Status('error')
        return
      }
      setPermissions(result.data.permissions)
      const allOk = result.data.permissions.every((p) => p.status === 'ok' || p.status === 'warn')
      setStep3Status(allOk ? 'done' : 'active')
      if (allOk) doFetchAccountInfo()
    } catch {
      setError('Permission check failed')
      setStep3Status('error')
    }
  }

  // Fetch account info
  const doFetchAccountInfo = async () => {
    setStep4Status('verifying')
    try {
      const result = await fetchAccountInfo()
      if (!result.success) {
        setError(result.error || 'Failed to fetch account info')
        setStep4Status('error')
        return
      }
      setAccountInfo(result.data!)
      setStep4Status('done')
    } catch {
      setError('Failed to fetch account info')
      setStep4Status('error')
    }
  }

  // Verify: Step 1 save → done, then auto-start Step 2
  const verifyCredentials = async () => {
    const key = apiKey().trim()
    const secret = apiSecret().trim()
    if (!key || !secret) return

    setStep1Status('verifying')
    setError('')

    try {
      const result = await saveCredential({
        exchange: 'binance',
        api_key: key,
        api_secret: secret,
        market_type: 'perpetual',
        label: 'binance verification',
      })
      if (!result.success) {
        throw new Error(result.error || 'Failed to save credentials')
      }
      setStep1Status('done')
      doTestConnectivity()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save credentials')
      setStep1Status('error')
    }
  }

  const handleContinue = () => {
    updateWizard({ exchange: 'binance' })
    // No longer store exchange_api_key/secret in memory — already saved to backend DB.
    // All subsequent exchange operations use the saved credential via backend.
    advanceStep(WizardStep.ConfigureParams)
    navigate('/setup/params', { replace: true })
  }

  const canContinue = () => step2Status() === 'done' && step3Status() === 'done' && step4Status() === 'done'

  return (
    <WizardLayout
      step={WizardStep.SelectExchange}
      title="Connect Binance"
      subtitle="Provide your API credentials"
      actions={
        <div class="flex gap-3">
          <button
            onClick={() => navigate('/setup/llm', { replace: true })}
            class="px-5 py-2.5 text-sm text-white/40 hover:text-white/60 rounded-xl transition-colors duration-200"
          >
            Back
          </button>
          <button
            onClick={handleContinue}
            disabled={!canContinue()}
            class="px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
          >
            Continue
          </button>
        </div>
      }
    >
      <FlowSteps steps={steps} statuses={statuses()} summaries={summaries()} />
    </WizardLayout>
  )
}

export default SelectExchange
