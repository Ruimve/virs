import { useEffect, useContext } from 'react'
import { useNavigate } from 'react-router-dom'
import { WizardContext } from './context'
import type { WizardStepValue } from './consts'

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
  const context = useContext(WizardContext);
  if (!context) {
    throw new Error('useWizard 必须在 WizardProvider 内部使用');
  }
  return context;
}