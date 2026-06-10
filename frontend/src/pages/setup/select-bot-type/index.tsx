import { type Component, createSignal, Show } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import WizardLayout from '../../../components/WizardLayout'
import { updateWizard, advanceStep, WizardStep, getWizardState } from '../../../lib/wizard'

const BOT_TYPES = [
  {
    id: 'grid' as const,
    name: 'Grid Bot',
    desc: 'Profit from market volatility by placing orders at regular price intervals. Best for ranging markets.',
    features: ['Grid levels', 'Auto buy/sell', 'Range-bound profit'],
    color: 'from-indigo-500/20 to-blue-500/20',
    border: 'border-indigo-500/30',
    icon: (
      <svg class="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 6A2.25 2.25 0 016 3.75h2.25A2.25 2.25 0 0110.5 6v2.25a2.25 2.25 0 01-2.25 2.25H6a2.25 2.25 0 01-2.25-2.25V6zM3.75 15.75A2.25 2.25 0 016 13.5h2.25a2.25 2.25 0 012.25 2.25V18a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 18v-2.25zM13.5 6a2.25 2.25 0 012.25-2.25H18A2.25 2.25 0 0120.25 6v2.25A2.25 2.25 0 0118 10.5h-2.25a2.25 2.25 0 01-2.25-2.25V6zM13.5 15.75a2.25 2.25 0 012.25-2.25H18a2.25 2.25 0 012.25 2.25V18A2.25 2.25 0 0118 20.25h-2.25a2.25 2.25 0 01-2.25-18v-2.25z" />
      </svg>
    ),
  },
  {
    id: 'auto' as const,
    name: 'Auto Bot',
    desc: 'AI-driven fully automated trading. Analyzes market conditions and executes trades autonomously.',
    features: ['AI analysis', 'Auto execution', 'Multi-market'],
    color: 'from-violet-500/20 to-purple-500/20',
    border: 'border-violet-500/30',
    icon: (
      <svg class="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M15.59 14.37a6 6 0 01-5.84 7.38v-4.8m5.84-2.58a14.98 14.98 0 006.16-12.12A14.98 14.98 0 009.631 8.41m5.96 5.96a14.926 14.926 0 01-5.841 2.58m-.119-8.54a6 6 0 00-7.381 5.84h4.8m2.581-5.84a14.927 14.927 0 00-2.58 5.84m2.699 2.7c-.103.021-.207.041-.311.06a15.09 15.09 0 01-2.448-2.448 14.9 14.9 0 01.06-.312m-2.24 2.39a4.493 4.493 0 00-1.757 4.306 4.493 4.493 0 004.306-1.758M16.5 9a1.5 1.5 0 11-3 0 1.5 1.5 0 013 0z" />
      </svg>
    ),
  },
]

const SelectBotType: Component = () => {
  const navigate = useNavigate()
  const wizard = getWizardState()
  const [selected, setSelected] = createSignal<'grid' | 'auto' | ''>(wizard().bot_type || '')

  const canContinue = () => selected().length > 0

  const handleContinue = () => {
    updateWizard({ bot_type: selected() })
    advanceStep(WizardStep.ConfigureLlm)
    navigate('/setup/llm', { replace: true })
  }

  return (
    <WizardLayout
      step={WizardStep.SelectBotType}
      title="Choose Strategy"
      subtitle="Select the trading strategy that fits your goals"
      actions={
        <button
          onClick={handleContinue}
          disabled={!canContinue()}
          class="px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
        >
          Continue
        </button>
      }
    >
      {/* Bot type selection */}
      <div class="space-y-4 mb-8">
        {BOT_TYPES.map((bot) => {
          const isSelected = () => selected() === bot.id
          return (
            <button
              onClick={() => setSelected(bot.id)}
              class={`group w-full p-5 rounded-xl border text-left transition-all duration-200 ${
                isSelected()
                  ? `bg-gradient-to-br ${bot.color} ${bot.border} ring-1 ring-white/10`
                  : 'bg-white/[0.02] border-white/[0.06] hover:bg-white/[0.04] hover:border-white/[0.1]'
              }`}
            >
              <div class="flex items-start gap-4">
                <div class={`shrink-0 ${isSelected() ? 'text-indigo-400' : 'text-white/20'}`}>
                  {bot.icon}
                </div>
                <div class="flex-1 min-w-0">
                  <p class={`text-base font-medium ${isSelected() ? 'text-white/90' : 'text-white/60'}`}>
                    {bot.name}
                  </p>
                  <p class="text-[13px] text-white/30 mt-1 leading-relaxed">{bot.desc}</p>
                  <div class="flex gap-2 mt-3">
                    {bot.features.map((f) => (
                      <span class={`px-2 py-0.5 rounded-md text-[11px] ${
                        isSelected() ? 'bg-white/10 text-white/50' : 'bg-white/[0.03] text-white/20'
                      }`}>
                        {f}
                      </span>
                    ))}
                  </div>
                </div>
                <Show when={isSelected()}>
                  <div class="shrink-0 w-5 h-5 rounded-full bg-indigo-500 flex items-center justify-center">
                    <svg class="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                    </svg>
                  </div>
                </Show>
              </div>
            </button>
          )
        })}
      </div>
    </WizardLayout>
  )
}

export default SelectBotType
