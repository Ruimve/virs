import type { ReactNode } from 'react';
import { WizardStep } from './consts';
import { STEP_LABELS, TOTAL_SETUP_STEPS } from './consts';

interface WizardLayoutProps {
  step: number;
  title: string;
  subtitle?: string;
  children: ReactNode;
  actions?: ReactNode;
}

export const Wizard = ({ step, title, subtitle, children, actions }: WizardLayoutProps) => {
  const stepIndex = step - WizardStep.SelectBotType + 1;

  return (
    <div className=" h-full flex flex-col justify-between">
      <div className="flex-1 flex justify-center relative z-10 overflow-y-auto">
        <div className="w-full max-w-lg px-4 md:px-8 pt-8 md:pt-16 pb-6">
          <div className="mb-8 md:mb-10">
            <p className="text-caption tracking-[0.2em] text-accent/60 mb-2 md:mb-3 uppercase">
              Step {stepIndex} of {TOTAL_SETUP_STEPS} — {STEP_LABELS[step]}
            </p>
            <h2 className="text-xl md:text-2xl font-extralight tracking-wide text-on-base">
              {title}
            </h2>
            {subtitle && (
              <p className="mt-1.5 md:mt-2 text-sm text-on-surface-tertiary">{subtitle}</p>
            )}
          </div>

          {children}
        </div>
      </div>

      {}
      {actions && (
        <div className="shrink-0 z-10 px-4 md:px-8 h-auto md:h-20 py-3 md:py-0 border-t border-line-subtle bg-base flex items-center">
          <div className="flex flex-col-reverse sm:flex-row gap-2 sm:gap-3 sm:justify-end w-full">
            {actions}
          </div>
        </div>
      )}
    </div>
  );
};
