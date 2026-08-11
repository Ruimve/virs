import { useState, useEffect, useMemo, useCallback, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import { findActiveBot } from '@/service';
import { Button } from '@/components/Button';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard } from '../context/WizardContext';
import { WizardStep } from '../context/WizardContext/define';
import { ExistingBot } from './ExistingBot';
import { NoBot } from './NoBot';

const SelectBotType = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { updateWizard, advanceStep } = useWizard();
  const [existingBot, setExistingBot] = useState<{ id: string } | null>(null);
  const [botType, setBotType] = useState<'auto' | 'manual'>('auto');

  useEffect(() => {
    updateWizard({ current_step: WizardStep.SelectBotType });
    findActiveBot().then((bot) => {
      if (bot) setExistingBot(bot);
    });
  }, [updateWizard]);

  const handleContinue = useCallback(() => {
    updateWizard({ bot_type: botType });
    advanceStep(WizardStep.ConfigureLlm);
    startTransition(() => {
      navigate('/setup/llm', { replace: true });
    });
  }, [botType, updateWizard, advanceStep, navigate]);

  const handleChangeBotType = useCallback((bt: 'auto' | 'manual') => {
    setBotType(bt);
  }, []);

  const actions = useMemo(() => {
    if (existingBot?.id) return null;

    const disabled = botType !== 'auto';
    return (
      <Button variant="primary" onClick={handleContinue} disabled={disabled} loading={isPending}>
        Continue
      </Button>
    );
  }, [existingBot?.id, botType, isPending, handleContinue]);

  return (
    <Wizard
      step={WizardStep.SelectBotType}
      title="Choose Strategy"
      subtitle="Select the trading strategy that fits your goals"
      actions={actions}
    >
      {existingBot?.id ? (
        <ExistingBot botId={existingBot?.id} />
      ) : (
        <NoBot botType={botType} changeBotType={handleChangeBotType} />
      )}
    </Wizard>
  );
};

export default SelectBotType;
