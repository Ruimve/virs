import { createContext, useContext, useLayoutEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import type { WizardState, WizardStepValue } from './define';

export interface WizardContextType {
  wizard: WizardState;
  updateWizard: (partial: Partial<WizardState>) => void;
  advanceStep: (step?: WizardStepValue) => void;
  resetWizard: () => void;
}

export const WizardContext = createContext<WizardContextType | null>(null);

export const useWizardGuard = (currentStep: WizardStepValue, requiredStep: WizardStepValue) => {
  const navigate = useNavigate();
  useLayoutEffect(() => {
    if (currentStep < requiredStep) {
      navigate('/setup/bot-type', { replace: true });
    }
  }, [currentStep, requiredStep, navigate]);
};

export const useWizard = () => {
  const context = useContext(WizardContext);
  if (!context) {
    throw new Error('useWizard 必须在 WizardProvider 内部使用');
  }
  return context;
};
