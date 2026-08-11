import { useCallback, useMemo, useState, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import { WizardStep } from '../context/WizardContext/consts';
import { Button } from '@/components/Button';
import { FormCard, FormField, FormSelect, ToggleSwitch } from '../components';

const INTERVAL_OPTIONS = [
  { label: '4 hours', value: '4h' },
  { label: '8 hours', value: '8h' },
  { label: '12 hours', value: '12h' },
  { label: '24 hours', value: '24h' },
];

const RISK_OPTIONS = [
  { label: 'Low — Conservative', value: 'low' },
  { label: 'Medium — Balanced', value: 'medium' },
  { label: 'High — Aggressive', value: 'high' },
];

const ConfigureOptimization = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.ConfigureOptimization);

  const [autoOptimize, setAutoOptimize] = useState(wizard.auto_optimize);
  const [interval, setInterval] = useState(wizard.optimization_interval || '8h');
  const [risk, setRisk] = useState(wizard.risk_tolerance || 'medium');

  const handleBack = useCallback(() => {
    navigate('/setup/params', { replace: true });
  }, [navigate]);

  const handleContinue = useCallback(() => {
    updateWizard({
      auto_optimize: autoOptimize,
      optimization_interval: interval,
      risk_tolerance: risk,
    });
    advanceStep(WizardStep.ReviewLaunch);
    startTransition(() => {
      navigate('/setup/review', { replace: true });
    });
  }, [autoOptimize, interval, risk, updateWizard, advanceStep, navigate]);

  const actions = useMemo(() => {
    return (
      <>
        <Button variant="ghost" onClick={handleBack}>
          Back
        </Button>
        <Button variant="primary" onClick={handleContinue} loading={isPending}>
          Continue
        </Button>
      </>
    );
  }, [isPending, handleBack, handleContinue]);

  return (
    <Wizard
      step={WizardStep.ConfigureOptimization}
      title="Auto-Optimization"
      subtitle="AI periodically evaluates and refines your strategy"
      actions={actions}
    >
      <FormCard>
        <FormField label="" noBorder={!autoOptimize}>
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="text-sm font-medium text-on-base">Auto-Optimization</div>
              <div className="text-xs text-on-surface-tertiary mt-0.5 leading-relaxed">
                AI refines strategy based on trade history
              </div>
            </div>
            <ToggleSwitch on={autoOptimize} onClick={() => setAutoOptimize(!autoOptimize)} />
          </div>
        </FormField>

        {autoOptimize && (
          <>
            <FormField label="Interval">
              <FormSelect value={interval} onChange={(e) => setInterval(e.target.value)}>
                {INTERVAL_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </FormSelect>
            </FormField>

            <FormField label="Risk Tolerance" noBorder>
              <FormSelect value={risk} onChange={(e) => setRisk(e.target.value)}>
                {RISK_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </FormSelect>
            </FormField>
          </>
        )}
      </FormCard>
      <p className="text-caption text-on-surface-muted mt-3 px-1 leading-relaxed">
        Runs hourly, evaluates closed trades. Optimized prompts are hot-swapped without restart.
      </p>
    </Wizard>
  );
};

export default ConfigureOptimization;
