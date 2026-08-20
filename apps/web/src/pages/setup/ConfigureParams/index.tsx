import { useCallback, useMemo, useState, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizardGuard, useWizard } from '../context/WizardContext';
import { WizardStep } from '../context/WizardContext/define';
import { Button } from '@/components/Button';
import { Input } from '@/components/Input';
import { FormCard, FormField, FormSelect, LeverageSlider } from '../components';

const TRADING_PAIRS = ['BTCUSDT', 'ETHUSDT', 'SOLUSDT', 'BNBUSDT'];

const TIMEFRAME_OPTIONS = [
  { label: '5 minutes', value: '300' },
  { label: '15 minutes', value: '900' },
  { label: '1 hour', value: '3600' },
  { label: '4 hours', value: '14400' },
  { label: '1 day', value: '86400' },
];

const DEFAULT_VALUES: Record<string, string> = {
  symbol: 'BTCUSDT',
  leverage: '10',
  max_position_pct: '20',
  decision_interval: '300',
};

const ConfigureParams = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.ConfigureParams);

  const [values, setValues] = useState<Record<string, string>>({
    ...DEFAULT_VALUES,
    ...wizard.bot_params,
  });

  const handleFieldChange = useCallback((key: string, val: string) => {
    setValues((prev) => ({ ...prev, [key]: val }));
  }, []);

  const handleLeverageChange = useCallback((val: number) => {
    setValues((prev) => ({ ...prev, leverage: String(val) }));
  }, []);

  const handleBack = useCallback(() => {
    navigate('/setup/exchange', { replace: true });
  }, [navigate]);

  const handleContinue = useCallback(() => {
    updateWizard({ bot_params: values });
    advanceStep(WizardStep.ConfigureOptimization);
    startTransition(() => {
      navigate('/setup/optimization', { replace: true });
    });
  }, [values, updateWizard, advanceStep, navigate]);

  const actions = useMemo(() => {
    const disabled = !values.symbol || !values.max_position_pct || !values.leverage;
    return (
      <>
        <Button variant="ghost" onClick={handleBack}>
          Back
        </Button>
        <Button variant="primary" onClick={handleContinue} disabled={disabled} loading={isPending}>
          Continue
        </Button>
      </>
    );
  }, [values, isPending, handleBack, handleContinue]);

  return (
    <>
      <title>交易参数 - VIRS</title>
      <Wizard
        step={WizardStep.ConfigureParams}
        title="Trading Parameters"
        subtitle="Configure pair, position size, leverage and timeframe"
        actions={actions}
      >
        <FormCard>
          <FormField label="Trading Pair" required>
            <Input
              mono
              value={values.symbol ?? ''}
              onChange={(e) => handleFieldChange('symbol', e.target.value)}
              list="pairList"
              placeholder="BTCUSDT"
            />
            <datalist id="pairList">
              {TRADING_PAIRS.map((p) => (
                <option key={p} value={p} />
              ))}
            </datalist>
          </FormField>

          <FormField label="Max Position %" required>
            <Input
              type="number"
              suffix="%"
              value={values.max_position_pct ?? ''}
              onChange={(e) => handleFieldChange('max_position_pct', e.target.value)}
              placeholder="20"
            />
          </FormField>

          <FormField label="Leverage">
            <div className="flex items-center justify-between mb-2">
              <span className="font-mono text-sm font-semibold text-accent">
                {values.leverage}x
              </span>
            </div>
            <LeverageSlider
              value={Number(values.leverage)}
              min={1}
              max={20}
              step={1}
              onChange={handleLeverageChange}
            />
          </FormField>

          <FormField label="Decision Interval">
            <FormSelect
              value={values.decision_interval || '300'}
              onChange={(e) => handleFieldChange('decision_interval', e.target.value)}
            >
              {TIMEFRAME_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </FormSelect>
          </FormField>
        </FormCard>
      </Wizard>
    </>
  );
};

export default ConfigureParams;
