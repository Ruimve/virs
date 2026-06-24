import { createContext, useContext, useEffect } from 'react';
import { type WizardState, type WizardStepValue } from './consts';
import { useNavigate } from 'react-router-dom';

export interface WizardContextType {
  wizard: WizardState;
  updateWizard: (partial: Partial<WizardState>) => void;
  advanceStep: (step?: WizardStepValue) => void;
  resetWizard: () => void;
}

export const WizardContext = createContext<WizardContextType | null>(null);

// ── 向导步骤守卫：直接访问非首页时跳转首页 ──
export function useWizardGuard(currentStep: WizardStepValue, requiredStep: WizardStepValue) {
  const navigate = useNavigate();
  useEffect(() => {
    if (currentStep < requiredStep) {
      navigate('/setup/bot-type', { replace: true });
    }
  }, [currentStep, requiredStep, navigate]);
}

export function useWizard() {
  const context = useContext(WizardContext);
  if (!context) {
    throw new Error('useWizard 必须在 WizardProvider 内部使用');
  }
  return context;
}
