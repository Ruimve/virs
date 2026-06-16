import type { ReactNode } from 'react'
import { WizardStep } from '../../lib/wizard'
import { useTheme } from '../../lib/theme'
import VirsLogo from '../VirsLogo'

interface WizardLayoutProps {
  step: number
  title: string
  subtitle?: string
  children: ReactNode
  actions?: ReactNode
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

function WizardLayout({ step, title, subtitle, children, actions }: WizardLayoutProps) {
  const { isDark, toggleTheme } = useTheme()
  const stepIndex = step - WizardStep.SelectBotType + 1

  return (
    <div className="h-screen bg-base flex flex-col relative overflow-hidden">
      {/* Background */}
      <div className="absolute inset-0 overflow-hidden">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      {/* Top bar */}
      <div className="relative z-10 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle">
        <VirsLogo />

        {/* Step indicator */}
        <div className="flex items-center gap-2">
          <span className="text-[11px] text-on-surface-tertiary md:hidden">
            Step {stepIndex}/{TOTAL_SETUP_STEPS}
          </span>
          <div className="hidden md:flex items-center gap-2">
            {Array.from({ length: TOTAL_SETUP_STEPS }, (_, i) => {
              const stepNum = i + 1
              const isActive = stepNum === stepIndex
              const isCompleted = stepNum < stepIndex
              return (
                <div key={i} className="flex items-center gap-2">
                  <div
                    className={`w-7 h-7 rounded-full flex items-center justify-center text-[11px] font-medium transition-all duration-300 ${
                      isActive
                        ? 'bg-indigo-500/80 text-white'
                        : isCompleted
                        ? 'bg-indigo-500/20 text-indigo-400 border border-indigo-500/30'
                        : 'bg-surface-1 text-on-surface-faint border border-line-default'
                    }`}
                  >
                    {isCompleted ? (
                      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2.5">
                        <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                      </svg>
                    ) : (
                      stepNum
                    )}
                  </div>
                  {i < TOTAL_SETUP_STEPS - 1 && (
                    <div className={`w-6 h-[1px] ${isCompleted ? 'bg-indigo-500/40' : 'bg-line-default'}`} />
                  )}
                </div>
              )
            })}
          </div>
        </div>

        <div className="flex items-center gap-1">
          <button
            onClick={toggleTheme}
            className="p-2 rounded-lg text-on-surface-tertiary hover:text-on-surface-secondary hover:bg-surface-2 transition-colors"
            title={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
          >
            {isDark ? (
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2">
                <path strokeLinecap="round" strokeLinejoin="round" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
              </svg>
            ) : (
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2">
                <path strokeLinecap="round" strokeLinejoin="round" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
              </svg>
            )}
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 flex justify-center relative z-10 overflow-y-auto">
        <div className="w-full max-w-lg px-4 md:px-8 pt-8 md:pt-16 pb-6">
          <div className="mb-8 md:mb-10">
            <p className="text-[11px] tracking-[0.2em] text-indigo-400/60 mb-2 md:mb-3 uppercase">
              Step {stepIndex} of {TOTAL_SETUP_STEPS} — {STEP_LABELS[step]}
            </p>
            <h2 className="text-xl md:text-2xl font-extralight tracking-wide text-on-base">{title}</h2>
            {subtitle && (
              <p className="mt-1.5 md:mt-2 text-sm text-on-surface-tertiary">{subtitle}</p>
            )}
          </div>

          {children}
        </div>
      </div>

      {/* Bottom actions */}
      {actions && (
        <div className="shrink-0 z-10 px-4 md:px-8 h-auto md:h-20 py-3 md:py-0 border-t border-line-subtle bg-base flex items-center">
          <div className="flex flex-col-reverse sm:flex-row gap-2 sm:gap-3 sm:justify-end w-full">
            {actions}
          </div>
        </div>
      )}
    </div>
  )
}

export default WizardLayout
