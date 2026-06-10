import { type Component, createSignal, For, Show } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import WizardLayout from '../../../components/WizardLayout'
import { updateWizard, advanceStep, WizardStep, getWizardState } from '../../../lib/wizard'
import type { MarketType } from '../../../lib/market-context'

const MARKET_TYPES: Array<{ id: MarketType; label: string; desc: string }> = [
  { id: 'perpetual', label: 'Perpetual', desc: 'USDT-M futures' },
  { id: 'spot', label: 'Spot', desc: 'Spot trading' },
]

// Grid bot parameters
const GRID_PARAMS = [
  { key: 'symbol', label: 'Trading Pair', type: 'text' as const, placeholder: 'BTC/USDT', required: true },
  { key: 'upper_price', label: 'Upper Price', type: 'number' as const, placeholder: '70000', required: true },
  { key: 'lower_price', label: 'Lower Price', type: 'number' as const, placeholder: '60000', required: true },
  { key: 'grid_levels', label: 'Grid Levels', type: 'number' as const, placeholder: '10', required: true },
  { key: 'investment', label: 'Investment (USDT)', type: 'number' as const, placeholder: '1000', required: true },
]

// Auto bot parameters
const AUTO_PARAMS = [
  { key: 'symbol', label: 'Trading Pair', type: 'text' as const, placeholder: 'BTC/USDT', required: true },
  { key: 'investment', label: 'Investment (USDT)', type: 'number' as const, placeholder: '1000', required: true },
  { key: 'max_position', label: 'Max Position', type: 'number' as const, placeholder: '5', required: false },
  { key: 'stop_loss_pct', label: 'Stop Loss %', type: 'number' as const, placeholder: '5', required: false },
  { key: 'take_profit_pct', label: 'Take Profit %', type: 'number' as const, placeholder: '10', required: false },
]

const ConfigureParams: Component = () => {
  const navigate = useNavigate()
  const wizard = getWizardState()
  const isGrid = () => wizard().bot_type === 'grid'
  const params = () => isGrid() ? GRID_PARAMS : AUTO_PARAMS

  const [values, setValues] = createSignal<Record<string, string>>(
    (wizard().bot_params as Record<string, string>) || {}
  )
  const [selectedMarket, setSelectedMarket] = createSignal<MarketType>(wizard().market_type || 'perpetual')

  const setValue = (key: string, val: string) => {
    setValues((prev) => ({ ...prev, [key]: val }))
  }

  const canContinue = () => {
    const v = values()
    return params().filter((p) => p.required).every((p) => v[p.key]?.trim())
  }

  const handleContinue = () => {
    updateWizard({ bot_params: values(), market_type: selectedMarket() })
    advanceStep(WizardStep.ReviewLaunch)
    navigate('/setup/review', { replace: true })
  }

  return (
    <WizardLayout
      step={WizardStep.ConfigureParams}
      title={isGrid() ? 'Grid Parameters' : 'Auto Trading Parameters'}
      subtitle="Configure the trading parameters for your bot"
      actions={
        <div class="flex gap-3">
          <button
            onClick={() => navigate('/setup/exchange', { replace: true })}
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
      <div class="space-y-4">
        {/* Market type toggle */}
        <div>
          <p class="text-[11px] tracking-[0.15em] text-white/25 uppercase mb-3">Market Type</p>
          <div class="flex gap-3">
            {MARKET_TYPES.map((mt) => {
              const isSelected = () => selectedMarket() === mt.id
              return (
                <button
                  onClick={() => setSelectedMarket(mt.id)}
                  class={`flex-1 p-3 rounded-xl border text-center transition-all duration-200 ${
                    isSelected()
                      ? 'bg-indigo-500/10 border-indigo-500/30 text-white/90'
                      : 'bg-white/[0.02] border-white/[0.06] text-white/40 hover:bg-white/[0.04]'
                  }`}
                >
                  <p class="text-sm font-medium">{mt.label}</p>
                  <p class="text-[11px] text-white/25 mt-0.5">{mt.desc}</p>
                </button>
              )
            })}
          </div>
        </div>

        <For each={params()}>
          {(param) => (
            <div>
              <label class="block text-[11px] tracking-[0.15em] text-white/25 uppercase mb-2">
                {param.label}
                <Show when={param.required}>
                  <span class="text-indigo-400/60 ml-1">*</span>
                </Show>
              </label>
              <input
                type={param.type}
                value={values()[param.key] || ''}
                onInput={(e) => setValue(param.key, e.currentTarget.value)}
                class="w-full px-4 py-3 bg-white/[0.04] border border-white/[0.08] rounded-xl text-sm text-white/90 placeholder-white/20 focus:outline-none focus:border-indigo-500/40 transition-all duration-200"
                placeholder={param.placeholder}
              />
            </div>
          )}
        </For>
      </div>
    </WizardLayout>
  )
}

export default ConfigureParams
