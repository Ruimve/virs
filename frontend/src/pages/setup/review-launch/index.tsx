import { type Component, createSignal, Show } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import WizardLayout from '../../../components/WizardLayout'
import { updateWizard, advanceStep, WizardStep, getWizardState } from '../../../lib/wizard'
import { createGridBot, createAutoBot, startGridBot, startAutoBot } from '../../../lib/api'

const ReviewLaunch: Component = () => {
  const navigate = useNavigate()
  const wizard = getWizardState()
  const isGrid = () => wizard().bot_type === 'grid'
  const v = () => (wizard().bot_params as Record<string, string>) || {}

  const [paperMode, setPaperMode] = createSignal(wizard().paper_mode)
  const [launching, setLaunching] = createSignal(false)
  const [launchError, setLaunchError] = createSignal('')

  const handleLaunch = async () => {
    setLaunching(true)
    setLaunchError('')

    const w = wizard()
    const paper = paperMode()
    const marketType = w.market_type

    try {
      // Credentials are already saved to backend DB during wizard steps
      // (ConfigureLlm saves AI creds, SelectExchange saves exchange creds).
      // No need to re-save from memory — proceed directly to bot creation.

      // Step 1: Create bot
      let botId: string
      if (isGrid()) {
        const result = await createGridBot({
          symbol: v().symbol || '',
          exchange: w.exchange,
          grid_count: parseInt(v().grid_levels || '10'),
          upper_price: parseFloat(v().upper_price || '0'),
          lower_price: parseFloat(v().lower_price || '0'),
          quantity_per_grid: parseFloat(v().investment || '0'),
          leverage: parseInt(v().leverage || '5'),
          name: `Grid ${v().symbol || 'Bot'}`,
          paper_mode: paper,
          market_type: marketType,
        })
        if (!result.success || !result.data?.id) {
          setLaunchError(`Failed to create grid bot: ${result.error || 'Unknown error'}`)
          return
        }
        botId = result.data.id
      } else {
        const result = await createAutoBot({
          symbol: v().symbol || '',
          exchange: w.exchange,
          market_type: marketType,
          leverage: parseInt(v().leverage || '10'),
          max_position_pct: parseFloat(v().max_position || '80'),
          name: `Auto ${v().symbol || 'Bot'}`,
          paper_mode: paper,
        })
        if (!result.success || !result.data?.id) {
          setLaunchError(`Failed to create auto bot: ${result.error || 'Unknown error'}`)
          return
        }
        botId = result.data.id
      }

      // Step 2: Start bot
      if (isGrid()) {
        const result = await startGridBot(botId)
        if (!result.success) {
          setLaunchError(`Bot created but failed to start: ${result.error || 'Unknown error'}`)
          return
        }
      } else {
        const result = await startAutoBot(botId)
        if (!result.success) {
          setLaunchError(`Bot created but failed to start: ${result.error || 'Unknown error'}`)
          return
        }
      }

      // Step 3: Navigate to health check
      updateWizard({ paper_mode: paper, bot_id: botId })
      advanceStep(WizardStep.HealthCheck)
      navigate('/setup/health', { replace: true })
    } catch (err) {
      setLaunchError(`Unexpected error: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      setLaunching(false)
    }
  }

  return (
    <WizardLayout
      step={WizardStep.ReviewLaunch}
      title="Review & Launch"
      subtitle="Confirm your configuration and launch the bot"
      actions={
        <>
          <button
            onClick={() => navigate('/setup/params', { replace: true })}
            disabled={launching()}
            class="w-full sm:w-auto sm:px-5 py-2.5 text-sm text-on-surface-tertiary hover:text-on-surface-secondary rounded-xl transition-colors duration-200 disabled:opacity-30"
          >
            Back
          </button>
          <button
            onClick={handleLaunch}
            disabled={launching()}
            class="w-full sm:w-auto sm:px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
          >
            <Show when={!launching()} fallback={
              <span class="flex items-center justify-center gap-2">
                <svg class="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                </svg>
                Launching...
              </span>
            }>
              Launch Bot
            </Show>
          </button>
        </>
      }
    >
      <div class="space-y-6">
        {/* Trading Mode */}
        <div class="p-4 rounded-xl border border-line-default bg-surface-1">
          <p class="text-[11px] tracking-[0.15em] text-on-surface-muted uppercase mb-3">Trading Mode</p>
          <div class="flex flex-col sm:flex-row gap-3">
            <button
              onClick={() => setPaperMode(true)}
              class={`flex-1 p-4 rounded-xl border text-left transition-all duration-200 ${
                paperMode()
                  ? 'bg-indigo-500/10 border-indigo-500/30 ring-1 ring-indigo-500/20'
                  : 'bg-surface-1 border-line-default hover:bg-surface-2'
              }`}
            >
              <div class="flex items-center gap-3">
                <div class={`w-8 h-8 rounded-lg flex items-center justify-center ${
                  paperMode() ? 'bg-indigo-500/20 text-indigo-400' : 'bg-surface-2 text-on-surface-faint'
                }`}>
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                  </svg>
                </div>
                <div>
                  <p class={`text-sm font-medium ${paperMode() ? 'text-on-base' : 'text-on-surface-tertiary'}`}>Paper Trading</p>
                  <p class="text-xs text-on-surface-muted mt-0.5">Simulated orders, no real funds at risk</p>
                </div>
              </div>
            </button>
            <button
              onClick={() => setPaperMode(false)}
              class={`flex-1 p-4 rounded-xl border text-left transition-all duration-200 ${
                !paperMode()
                  ? 'bg-amber-500/10 border-amber-500/30 ring-1 ring-amber-500/20'
                  : 'bg-surface-1 border-line-default hover:bg-surface-2'
              }`}
            >
              <div class="flex items-center gap-3">
                <div class={`w-8 h-8 rounded-lg flex items-center justify-center ${
                  !paperMode() ? 'bg-amber-500/20 text-amber-400' : 'bg-surface-2 text-on-surface-faint'
                }`}>
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
                  </svg>
                </div>
                <div>
                  <p class={`text-sm font-medium ${!paperMode() ? 'text-on-base' : 'text-on-surface-tertiary'}`}>Real Trading</p>
                  <p class="text-xs text-on-surface-muted mt-0.5">Live orders with real funds</p>
                </div>
              </div>
            </button>
          </div>
        </div>

        {/* Summary */}
        <div class="p-4 rounded-xl bg-surface-1 border border-line-default">
          <p class="text-[11px] tracking-[0.15em] text-on-surface-muted uppercase mb-3">Summary</p>
          <div class="space-y-1.5">
            <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
              <span class="text-[12px] text-on-surface-tertiary">Exchange</span>
              <span class="text-[12px] text-on-surface-secondary font-mono">{wizard().exchange}</span>
            </div>
            <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
              <span class="text-[12px] text-on-surface-tertiary">Strategy</span>
              <span class="text-[12px] text-on-surface-secondary font-mono">{isGrid() ? 'Grid Bot' : 'Auto Bot'}</span>
            </div>
            <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
              <span class="text-[12px] text-on-surface-tertiary">Market</span>
              <span class="text-[12px] text-on-surface-secondary font-mono">{wizard().market_type}</span>
            </div>
            <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
              <span class="text-[12px] text-on-surface-tertiary">Symbol</span>
              <span class="text-[12px] text-on-surface-secondary font-mono">{v().symbol || '-'}</span>
            </div>
            <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
              <span class="text-[12px] text-on-surface-tertiary">AI Model</span>
              <span class="text-[12px] text-on-surface-secondary font-mono">{wizard().llm_model}</span>
            </div>
            <Show when={isGrid()}>
              <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
                <span class="text-[12px] text-on-surface-tertiary">Grid Levels</span>
                <span class="text-[12px] text-on-surface-secondary font-mono">{v().grid_levels || '-'}</span>
              </div>
              <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
                <span class="text-[12px] text-on-surface-tertiary">Price Range</span>
                <span class="text-[12px] text-on-surface-secondary font-mono">{v().lower_price || '-'} ~ {v().upper_price || '-'}</span>
              </div>
            </Show>
            <Show when={isGrid()}>
              <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
                <span class="text-[12px] text-on-surface-tertiary">Investment</span>
                <span class="text-[12px] text-on-surface-secondary font-mono">{v().investment || '-'} USDT</span>
              </div>
            </Show>
            <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
              <span class="text-[12px] text-on-surface-tertiary">Leverage</span>
              <span class="text-[12px] text-on-surface-secondary font-mono">{v().leverage || '-'}x</span>
            </div>
            <div class="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
              <span class="text-[12px] text-on-surface-tertiary">Mode</span>
              <span class={`text-[12px] font-mono font-medium ${paperMode() ? 'text-indigo-400' : 'text-amber-400'}`}>{paperMode() ? 'Paper' : 'Real'}</span>
            </div>
          </div>
        </div>

        {/* Launch error */}
        <Show when={launchError()}>
          <div class="p-3 bg-red-500/10 border border-red-500/20 rounded-xl text-sm text-red-400">
            {launchError()}
          </div>
        </Show>
      </div>
    </WizardLayout>
  )
}

export default ReviewLaunch
