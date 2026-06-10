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
          leverage: 5,
          name: `Grid ${v().symbol || 'Bot'}`,
          paper_mode: paper,
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
          leverage: 10,
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
        <div class="flex gap-3">
          <button
            onClick={() => navigate('/setup/params', { replace: true })}
            disabled={launching()}
            class="px-5 py-2.5 text-sm text-white/40 hover:text-white/60 rounded-xl transition-colors duration-200 disabled:opacity-30"
          >
            Back
          </button>
          <button
            onClick={handleLaunch}
            disabled={launching()}
            class="px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
          >
            <Show when={!launching()} fallback={
              <span class="flex items-center gap-2">
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
        </div>
      }
    >
      <div class="space-y-6">
        {/* Trading Mode */}
        <div class="p-4 rounded-xl border border-white/[0.06] bg-white/[0.02]">
          <p class="text-[11px] tracking-[0.15em] text-white/25 uppercase mb-3">Trading Mode</p>
          <div class="flex gap-3">
            <button
              onClick={() => setPaperMode(true)}
              class={`flex-1 p-4 rounded-xl border text-left transition-all duration-200 ${
                paperMode()
                  ? 'bg-indigo-500/10 border-indigo-500/30 ring-1 ring-indigo-500/20'
                  : 'bg-white/[0.02] border-white/[0.06] hover:bg-white/[0.04]'
              }`}
            >
              <div class="flex items-center gap-3">
                <div class={`w-8 h-8 rounded-lg flex items-center justify-center ${
                  paperMode() ? 'bg-indigo-500/20 text-indigo-400' : 'bg-white/[0.04] text-white/20'
                }`}>
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                  </svg>
                </div>
                <div>
                  <p class={`text-sm font-medium ${paperMode() ? 'text-white/90' : 'text-white/50'}`}>Paper Trading</p>
                  <p class="text-xs text-white/30 mt-0.5">Simulated orders, no real funds at risk</p>
                </div>
              </div>
            </button>
            <button
              onClick={() => setPaperMode(false)}
              class={`flex-1 p-4 rounded-xl border text-left transition-all duration-200 ${
                !paperMode()
                  ? 'bg-amber-500/10 border-amber-500/30 ring-1 ring-amber-500/20'
                  : 'bg-white/[0.02] border-white/[0.06] hover:bg-white/[0.04]'
              }`}
            >
              <div class="flex items-center gap-3">
                <div class={`w-8 h-8 rounded-lg flex items-center justify-center ${
                  !paperMode() ? 'bg-amber-500/20 text-amber-400' : 'bg-white/[0.04] text-white/20'
                }`}>
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
                  </svg>
                </div>
                <div>
                  <p class={`text-sm font-medium ${!paperMode() ? 'text-white/90' : 'text-white/50'}`}>Real Trading</p>
                  <p class="text-xs text-white/30 mt-0.5">Live orders with real funds</p>
                </div>
              </div>
            </button>
          </div>
        </div>

        {/* Summary */}
        <div class="p-4 rounded-xl bg-white/[0.02] border border-white/[0.06]">
          <p class="text-[11px] tracking-[0.15em] text-white/25 uppercase mb-3">Summary</p>
          <div class="space-y-2 text-[13px]">
            <div class="flex justify-between">
              <span class="text-white/30">Exchange</span>
              <span class="text-white/60">{wizard().exchange}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-white/30">Strategy</span>
              <span class="text-white/60">{isGrid() ? 'Grid Bot' : 'Auto Bot'}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-white/30">Market</span>
              <span class="text-white/60">{wizard().market_type}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-white/30">Symbol</span>
              <span class="text-white/60">{v().symbol || '-'}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-white/30">AI Model</span>
              <span class="text-white/60">{wizard().llm_model}</span>
            </div>
            <Show when={isGrid()}>
              <div class="flex justify-between">
                <span class="text-white/30">Grid Levels</span>
                <span class="text-white/60">{v().grid_levels || '-'}</span>
              </div>
              <div class="flex justify-between">
                <span class="text-white/30">Price Range</span>
                <span class="text-white/60">{v().lower_price || '-'} ~ {v().upper_price || '-'}</span>
              </div>
            </Show>
            <div class="flex justify-between">
              <span class="text-white/30">Investment</span>
              <span class="text-white/60">{v().investment || '-'} USDT</span>
            </div>
            <div class="flex justify-between">
              <span class="text-white/30">Mode</span>
              <span class={`font-medium ${paperMode() ? 'text-indigo-400' : 'text-amber-400'}`}>{paperMode() ? 'Paper' : 'Real'}</span>
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
