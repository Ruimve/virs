import { useCallback, useMemo, useState, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizardGuard, useWizard } from '../context/WizardContext';
import { WizardStep } from '../context/WizardContext/consts';
import { Input } from '@/components/Input';
import { Button } from '@/components/Button';

// Grid bot parameters
const GRID_PARAMS = [
  {
    key: 'symbol',
    label: 'Trading Pair',
    type: 'text' as const,
    placeholder: 'BTCUSDT',
    required: true,
  },
  {
    key: 'upper_price',
    label: 'Upper Price',
    type: 'number' as const,
    placeholder: '70000',
    required: true,
  },
  {
    key: 'lower_price',
    label: 'Lower Price',
    type: 'number' as const,
    placeholder: '60000',
    required: true,
  },
  {
    key: 'grid_levels',
    label: 'Grid Levels',
    type: 'number' as const,
    placeholder: '10',
    required: true,
  },
  {
    key: 'investment',
    label: 'Investment (USDT)',
    type: 'number' as const,
    placeholder: '1000',
    required: true,
  },
  { key: 'leverage', label: 'Leverage', type: 'number' as const, placeholder: '5', required: true },
];

// Auto bot parameters
const AUTO_PARAMS = [
  {
    key: 'symbol',
    label: 'Trading Pair',
    type: 'text' as const,
    placeholder: 'BTCUSDT',
    required: true,
  },
  {
    key: 'leverage',
    label: 'Leverage',
    type: 'number' as const,
    placeholder: '10',
    required: true,
  },
  {
    key: 'decision_interval',
    label: 'Decision Interval (seconds)',
    type: 'number' as const,
    placeholder: '300',
    required: true,
  },
];

const ConfigureParams = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.ConfigureParams);

  const [values, setValues] = useState<Record<string, string>>(wizard.bot_params);

  const params = useMemo(() => {
    const isGrid = wizard.bot_type === 'grid';
    return isGrid ? GRID_PARAMS : AUTO_PARAMS;
  }, [wizard.bot_type]);

  const handleBack = useCallback(() => {
    navigate('/setup/exchange', { replace: true });
  }, [navigate]);

  const handleContinue = useCallback(() => {
    updateWizard({ bot_params: values });
    advanceStep(WizardStep.ReviewLaunch);
    startTransition(() => {
      navigate('/setup/review', { replace: true });
    });
  }, [values, updateWizard, advanceStep, navigate]);

  const actions = useMemo(() => {
    const disabled = params.filter((p) => p.required).some((p) => !values[p.key]);

    return (
      <>
        <Button variant="ghost" onClick={handleBack}>
          Back
        </Button>
        <Button onClick={handleContinue} disabled={disabled} loading={isPending}>
          Continue
        </Button>
      </>
    );
  }, [params, values, isPending, handleBack, handleContinue]);

  return (
    <Wizard
      step={WizardStep.ConfigureParams}
      title={'Trading Parameters'}
      subtitle="Configure the trading parameters for your bot"
      actions={actions}
    >
      <div className="space-y-4">
        {params.map((param) => (
          <div key={param.key}>
            <label className="block text-caption tracking-[0.15em] text-on-surface-muted uppercase mb-2">
              {param.label}
              {param.required && <span className="text-accent/60 ml-1">*</span>}
            </label>
            <Input
              type={param.type}
              value={values[param.key] ?? ''}
              onChange={(e) => {
                setValues((prev) => ({ ...prev, [param.key]: e.target.value }));
              }}
              placeholder={param.placeholder}
            />
          </div>
        ))}
      </div>
    </Wizard>
  );
};

export default ConfigureParams;
