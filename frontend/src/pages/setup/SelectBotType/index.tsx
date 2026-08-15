import { useState, useEffect, useMemo, useCallback, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/Button';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard } from '../context/WizardContext';
import { WizardStep } from '../context/WizardContext/define';
import { NoBot } from './NoBot';

const SelectBotType = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { updateWizard, advanceStep } = useWizard();

  const [botType, setBotType] = useState<'chat' | 'agent'>('chat');

  useEffect(() => {
    updateWizard({ current_step: WizardStep.SelectBotType });
  }, [updateWizard]);

  const handleContinue = useCallback(() => {
    updateWizard({ bot_type: botType });
    advanceStep(WizardStep.ConfigureLlm);
    startTransition(() => {
      navigate('/setup/llm', { replace: true });
    });
  }, [botType, updateWizard, advanceStep, navigate]);

  const handleChangeBotType = useCallback((bt: 'chat' | 'agent') => {
    setBotType(bt);
  }, []);

  const actions = useMemo(() => {
    const disabled = botType !== 'chat';
    return (
      <Button variant="primary" onClick={handleContinue} disabled={disabled} loading={isPending}>
        Continue
      </Button>
    );
  }, [botType, isPending, handleContinue]);

  return (
    <>
      <title>选择策略 - VIRS</title>
      <Wizard
        step={WizardStep.SelectBotType}
        title="Choose Strategy"
        subtitle="Select the trading strategy that fits your goals"
        actions={actions}
      >
        {/* {bot?.id ? (
          <ExistingBot botId={bot?.id} />
        ) : ( */}
        <NoBot botType={botType} changeBotType={handleChangeBotType} />
        {/* )} */}
      </Wizard>
    </>
  );
};

export default SelectBotType;
