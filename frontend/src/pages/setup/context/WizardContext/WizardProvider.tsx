import { useCallback, useMemo, useState, type ReactNode } from 'react';
import { DEFAULT_STATE, type WizardState, type WizardStepValue } from './consts';
import { WizardContext } from '.';

export const WizardProvider = ({ children }: { children: ReactNode }) => {
  const [wizard, setWizard] = useState(DEFAULT_STATE);

  const updateWizard = useCallback((partial: Partial<WizardState>) => {
    setWizard((prev) => ({ ...prev, ...partial }));
  }, []);

  const resetWizard = useCallback(() => {
    setWizard(DEFAULT_STATE);
  }, []);

  const advanceStep = useCallback((step?: WizardStepValue) => {
    setWizard((prev) => ({
      ...prev,
      current_step: step ?? ((prev.current_step + 1) as WizardStepValue),
    }));
  }, []);

  const value = useMemo(() => {
    return {
      wizard,
      updateWizard,
      resetWizard,
      advanceStep,
    };
  }, [wizard, updateWizard, resetWizard, advanceStep]);

  return <WizardContext.Provider value={value}>{children}</WizardContext.Provider>;
};
