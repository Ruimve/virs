import { useCallback, useMemo, useState, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import { ShieldCheck, InfoCircle } from '@/components/Icon';
import { Button } from '@/components/Button';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import { WizardStep } from '../context/WizardContext/consts';

const ConfigureOptimization = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.ConfigureOptimization);

  const [autoOptimize, setAutoOptimize] = useState(wizard.auto_optimize);

  const handleBack = useCallback(() => {
    navigate('/setup/params', { replace: true });
  }, [navigate]);

  const handleContinue = useCallback(() => {
    updateWizard({ auto_optimize: autoOptimize });
    advanceStep(WizardStep.ReviewLaunch);
    startTransition(() => {
      navigate('/setup/review', { replace: true });
    });
  }, [autoOptimize, updateWizard, advanceStep, navigate]);

  const actions = useMemo(() => {
    return (
      <>
        <Button variant="ghost" onClick={handleBack}>
          Back
        </Button>
        <Button onClick={handleContinue} loading={isPending}>
          Continue
        </Button>
      </>
    );
  }, [isPending, handleBack, handleContinue]);

  return (
    <Wizard
      step={WizardStep.ConfigureOptimization}
      title="Strategy Auto-Optimization"
      subtitle="Let the AI continuously evaluate and optimize your trading strategy"
      actions={actions}
    >
      <div className="space-y-4">
        <div className="flex flex-col sm:flex-row gap-3">
          {}
          <div
            onClick={() => setAutoOptimize(true)}
            className={`flex-1 p-5 rounded-xl border text-left transition-all duration-300 cursor-pointer backdrop-blur-sm ${
              autoOptimize
                ? 'bg-accent/[0.06] border-accent/30 shadow-md shadow-accent/5'
                : 'bg-surface-1/40 border-line-default hover:bg-surface-2/40 hover:border-line-strong'
            }`}
          >
            <div className="flex items-start gap-3.5">
              <div
                className={`shrink-0 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-300 ${
                  autoOptimize
                    ? 'bg-accent/[0.12] text-accent'
                    : 'bg-surface-2/50 text-on-surface-faint'
                }`}
              >
                <ShieldCheck className="w-5 h-5" strokeWidth={1.8} />
              </div>
              <div className="flex-1">
                <p
                  className={`text-sm font-medium transition-colors duration-200 ${
                    autoOptimize ? 'text-on-base' : 'text-on-surface-tertiary'
                  }`}
                >
                  Enable Auto-Optimization
                </p>
                <p className="text-xs text-on-surface-muted mt-1 leading-relaxed">
                  The AI will periodically evaluate strategy performance and automatically generate
                  improved prompts based on trade history
                </p>
              </div>
              {autoOptimize && (
                <div className="shrink-0 w-5 h-5 rounded-full bg-accent flex items-center justify-center mt-0.5">
                  <svg
                    className="w-3 h-3 text-white"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth={3}
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                  </svg>
                </div>
              )}
            </div>
          </div>

          {}
          <div
            onClick={() => setAutoOptimize(false)}
            className={`flex-1 p-5 rounded-xl border text-left transition-all duration-300 cursor-pointer backdrop-blur-sm ${
              !autoOptimize
                ? 'bg-surface-2/30 border-line-default shadow-md'
                : 'bg-surface-1/40 border-line-default hover:bg-surface-2/40 hover:border-line-strong'
            }`}
          >
            <div className="flex items-start gap-3.5">
              <div
                className={`shrink-0 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-300 ${
                  !autoOptimize
                    ? 'bg-surface-2/50 text-on-surface-secondary'
                    : 'bg-surface-2/50 text-on-surface-faint'
                }`}
              >
                <InfoCircle className="w-5 h-5" strokeWidth={1.8} />
              </div>
              <div className="flex-1">
                <p
                  className={`text-sm font-medium transition-colors duration-200 ${
                    !autoOptimize ? 'text-on-base' : 'text-on-surface-tertiary'
                  }`}
                >
                  Skip
                </p>
                <p className="text-xs text-on-surface-muted mt-1 leading-relaxed">
                  Use the selected strategy as-is without automatic optimization. You can enable
                  this later
                </p>
              </div>
              {autoOptimize && (
                <div className="shrink-0 w-5 h-5 rounded-full bg-surface-2/50 flex items-center justify-center mt-0.5" />
              )}
            </div>
          </div>
        </div>

        {autoOptimize && (
          <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-accent/[0.04] border border-accent/20">
            <InfoCircle className="w-4 h-4 text-accent shrink-0 mt-0.5" strokeWidth={1.8} />
            <p className="text-xs text-on-surface-muted leading-relaxed">
              The strategy engine runs hourly, evaluating at least 10 closed trades. Low-scoring
              strategies are automatically refined by the AI. Optimized prompts are hot-swapped
              without restarting your bot.
            </p>
          </div>
        )}
      </div>
    </Wizard>
  );
};

export default ConfigureOptimization;
