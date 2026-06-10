import { type Component, type JSX, Show } from 'solid-js'
import { WizardStep } from '../../lib/wizard'

interface WizardLayoutProps {
  step: number
  title: string
  subtitle?: string
  children: JSX.Element
  actions?: JSX.Element
}

const STEP_LABELS: Record<number, string> = {
  [WizardStep.SelectBotType]: 'Strategy',
  [WizardStep.ConfigureLlm]: 'AI Model',
  [WizardStep.SelectExchange]: 'Exchange',
  [WizardStep.ConfigureParams]: 'Parameters',
  [WizardStep.ReviewLaunch]: 'Review',
  [WizardStep.HealthCheck]: 'Health',
}

const TOTAL_SETUP_STEPS = 6

const WizardLayout: Component<WizardLayoutProps> = (props) => {
  const stepIndex = () => props.step - WizardStep.SelectBotType + 1

  return (
    <div class="min-h-screen bg-[#0a0a0f] flex flex-col relative overflow-hidden">
      {/* Background */}
      <div class="absolute inset-0 overflow-hidden">
        <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      {/* Top bar */}
      <div class="relative z-10 flex items-center justify-between px-8 h-16 border-b border-white/[0.04]">
        <div class="flex items-center gap-3">
          <div class="w-8 h-8 rounded-lg bg-gradient-to-br from-indigo-500/20 to-violet-500/20 border border-indigo-500/20 flex items-center justify-center">
            <span class="text-sm font-extralight tracking-[0.2em] text-white/90">V</span>
          </div>
          <span class="text-sm font-extralight tracking-[0.2em] text-white/60">VIRS</span>
        </div>

        {/* Step indicator */}
        <div class="flex items-center gap-2">
          {Array.from({ length: TOTAL_SETUP_STEPS }, (_, i) => {
            const stepNum = i + 1
            const isActive = stepNum === stepIndex()
            const isCompleted = stepNum < stepIndex()
            return (
              <div class="flex items-center gap-2">
                <div
                  class={`w-7 h-7 rounded-full flex items-center justify-center text-[11px] font-medium transition-all duration-300 ${
                    isActive
                      ? 'bg-indigo-500/80 text-white'
                      : isCompleted
                      ? 'bg-indigo-500/20 text-indigo-400 border border-indigo-500/30'
                      : 'bg-white/[0.04] text-white/20 border border-white/[0.06]'
                  }`}
                >
                  {isCompleted ? (
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                    </svg>
                  ) : (
                    stepNum
                  )}
                </div>
                <Show when={i < TOTAL_SETUP_STEPS - 1}>
                  <div class={`w-6 h-[1px] ${isCompleted ? 'bg-indigo-500/40' : 'bg-white/[0.06]'}`} />
                </Show>
              </div>
            )
          })}
        </div>

        <div class="w-20" />
      </div>

      {/* Content */}
      <div class="flex-1 flex items-center justify-center relative z-10">
        <div class="w-full max-w-lg px-8">
          <div class="mb-10">
            <p class="text-[11px] tracking-[0.2em] text-indigo-400/60 mb-3 uppercase">
              Step {stepIndex()} of {TOTAL_SETUP_STEPS} — {STEP_LABELS[props.step]}
            </p>
            <h2 class="text-2xl font-extralight tracking-wide text-white/90">{props.title}</h2>
            <Show when={props.subtitle}>
              <p class="mt-2 text-sm text-white/30">{props.subtitle}</p>
            </Show>
          </div>

          {props.children}
        </div>
      </div>

      {/* Bottom actions */}
      <Show when={props.actions}>
        <div class="relative z-10 flex items-center justify-end px-8 h-20 border-t border-white/[0.04]">
          {props.actions}
        </div>
      </Show>
    </div>
  )
}

export default WizardLayout
