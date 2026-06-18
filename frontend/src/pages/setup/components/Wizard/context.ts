import { createContext } from "react";
import type { WizardState, WizardStepValue } from "./consts";

export interface WizardContextType {
  wizard: WizardState;
  updateWizard: (partial: Partial<WizardState>) => void;
  advanceStep: (step?: WizardStepValue) => void;
  resetWizard: () => void;
}

export const WizardContext = createContext<WizardContextType | null>(null);