import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react'
import { DEFAULT_STATE, type WizardState, type WizardStepValue } from './consts'
import { useNavigate } from 'react-router-dom'

export interface WizardContextType {
  wizard: WizardState
  updateWizard: (partial: Partial<WizardState>) => void
  advanceStep: (step?: WizardStepValue) => void
  resetWizard: () => void
}

export const WizardContext = createContext<WizardContextType | null>(null)

export function WizardProvider({ children }: { children: ReactNode }) {
  const [wizard, setWizard] = useState(DEFAULT_STATE)

  const updateWizard = useCallback((partial: Partial<WizardState>) => {
    setWizard((prev) => ({ ...prev, ...partial }))
  }, [])

  const resetWizard = useCallback(() => {
    setWizard(DEFAULT_STATE)
  }, [])

  const advanceStep = useCallback((step?: WizardStepValue) => {
    setWizard((prev) => ({
      ...prev,
      current_step: step ?? ((prev.current_step + 1) as WizardStepValue),
    }))
  }, [])

  return (
    <WizardContext.Provider
      value={{
        wizard,
        updateWizard,
        resetWizard,
        advanceStep,
      }}
    >
      {children}
    </WizardContext.Provider>
  )
}

// ── 向导步骤守卫：直接访问非首页时跳转首页 ──
export function useWizardGuard(currentStep: WizardStepValue, requiredStep: WizardStepValue) {
  const navigate = useNavigate()
  useEffect(() => {
    if (currentStep < requiredStep) {
      navigate('/setup/bot-type', { replace: true })
    }
  }, [requiredStep, navigate])
}

export function useWizard() {
  const context = useContext(WizardContext)
  if (!context) {
    throw new Error('useWizard 必须在 WizardProvider 内部使用')
  }
  return context
}
