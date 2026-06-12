import { type Component, type JSX, Show } from 'solid-js'
import { WizardStep } from '../../lib/wizard'
import { isDark, toggleTheme } from '../../lib/theme'

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
    <div class="h-screen bg-base flex flex-col relative overflow-hidden">
      {/* Background */}
      <div class="absolute inset-0 overflow-hidden">
        <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      {/* Top bar */}
      <div class="relative z-10 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle">
        <div class="flex items-center gap-2 md:gap-3">
          <div class="w-7 h-7 md:w-8 md:h-8 rounded-lg bg-gradient-to-br from-indigo-500/20 to-violet-500/20 border border-indigo-500/20 flex items-center justify-center">
            <span class="text-xs md:text-sm font-extralight tracking-[0.2em] text-on-base">V</span>
          </div>
          <span class="text-xs md:text-sm font-extralight tracking-[0.2em] text-on-surface-secondary hidden sm:inline">VIRS</span>
        </div>

        {/* Step indicator — compact on mobile, full dots on md+ */}
        <div class="flex items-center gap-2">
          {/* Mobile: text only */}
          <span class="text-[11px] text-on-surface-tertiary md:hidden">
            Step {stepIndex()}/{TOTAL_SETUP_STEPS}
          </span>
          {/* Desktop: full dots */}
          <div class="hidden md:flex items-center gap-2">
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
                        : 'bg-surface-1 text-on-surface-faint border border-line-default'
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
                    <div class={`w-6 h-[1px] ${isCompleted ? 'bg-indigo-500/40' : 'bg-line-default'}`} />
                  </Show>
                </div>
              )
            })}
          </div>
        </div>

        <div class="flex items-center gap-1">
          <button
            onClick={toggleTheme}
            class="p-2 rounded-lg text-on-surface-tertiary hover:text-on-surface-secondary hover:bg-surface-2 transition-colors"
            title={isDark() ? 'Switch to light mode' : 'Switch to dark mode'}
          >
            <Show when={isDark()} fallback={
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
              </svg>
            }>
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
              </svg>
            </Show>
          </button>
        </div>
      </div>

      {/* Content */}
      <div class="flex-1 flex justify-center relative z-10 overflow-y-auto">
        <div class="w-full max-w-lg px-4 md:px-8 pt-8 md:pt-16 pb-6">
          <div class="mb-8 md:mb-10">
            <p class="text-[11px] tracking-[0.2em] text-indigo-400/60 mb-2 md:mb-3 uppercase">
              Step {stepIndex()} of {TOTAL_SETUP_STEPS} — {STEP_LABELS[props.step]}
            </p>
            <h2 class="text-xl md:text-2xl font-extralight tracking-wide text-on-base">{props.title}</h2>
            <Show when={props.subtitle}>
              <p class="mt-1.5 md:mt-2 text-sm text-on-surface-tertiary">{props.subtitle}</p>
            </Show>
          </div>

          {props.children}
        </div>
      </div>

      {/* Bottom actions */}
      <Show when={props.actions}>
        <div class="shrink-0 z-10 px-4 md:px-8 h-auto md:h-20 py-3 md:py-0 border-t border-line-subtle bg-base flex items-center">
          <div class="flex flex-col-reverse sm:flex-row gap-2 sm:gap-3 sm:justify-end w-full">
            {props.actions}
          </div>
        </div>
      </Show>
    </div>
  )
}

export default WizardLayout
