import { useState, useCallback, useRef, type ReactNode, useMemo, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  saveAiCredential,
  fetchAiModels,
  fetchAiBalance,
  testAiCredential,
  type DeepSeekModel,
  type BalanceInfo,
} from '@/service';
import { FlowSteps, type FlowStepConfig, type FlowStepStatus } from '@/components/FlowStep';
import { Input } from '@/components/Input';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import { WizardStep } from '../context/WizardContext/consts';
import { Button } from '@/components/Button';

const ConfigureLlm = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.ConfigureLlm);

  const [step1Status, setStep1Status] = useState<FlowStepStatus>('active');
  const [step1Error, setStep1Error] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState('');
  const [models, setModels] = useState<DeepSeekModel[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [model, setModel] = useState(wizard.llm_model);

  const [step2Status, setStep2Status] = useState<FlowStepStatus>('pending');
  const [step2Error, setStep2Error] = useState<string | null>(null);

  const [step3Status, setStep3Status] = useState<FlowStepStatus>('pending');
  const [step3Error, setStep3Error] = useState<string | null>(null);
  const [balance, setBalance] = useState<BalanceInfo | null>(null);

  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const resetSteps = useCallback(() => {
    setStep1Status('active');
    setStep1Error(null);
    setStep2Status('pending');
    setStep2Error(null);
    setStep3Status('pending');
    setStep3Error(null);
  }, []);

  const startStep3 = useCallback(async () => {
    setStep3Status('verifying');
    try {
      const result = await fetchAiBalance();
      if (result.success && result.data?.balances && result.data.balances.length > 0) {
        const bal = result.data.balances[0];
        setBalance(bal);
        const total = parseFloat(bal.total_balance || '0');
        setStep3Status(total > 0 ? 'done' : 'active');
        return;
      }

      setStep3Status('error');
      setStep3Error('No balance info available. Ensure your account has credits.');
    } catch {
      setStep3Error('Network error');
      setStep3Status('error');
    }
  }, []);

  const startStep2 = useCallback(async () => {
    setStep2Status('verifying');
    try {
      const res = await testAiCredential();
      if (res.success && res.data?.connected) {
        setStep2Status('done');
        startStep3();
        return;
      }
      setStep2Error(res.error || res.data?.message || 'Connection failed');
      setStep2Status('error');
    } catch {
      setStep2Status('error');
      setStep2Error('Network error');
    }
  }, [startStep3]);

  const startStep1 = useCallback(async () => {
    setStep1Status('verifying');
    setStep1Error(null);
    try {
      const res = await saveAiCredential({
        provider: 'deepseek',
        api_key: apiKey,
        model: model,
        is_default: true,
      });
      if (res.success) {
        setStep1Status('done');
        startStep2();
        return;
      }

      setStep1Status('error');
      setStep1Error(res.error || 'Failed to save API key');
    } catch {
      setStep1Status('error');
      setStep1Error('Network error');
    }
  }, [apiKey, model, startStep2]);

  const fetchModels = useCallback(async () => {
    try {
      const result = await fetchAiModels();
      if (result.success) {
        const list = result.data?.models || [];
        setModels(list);
        if (list.length > 0) {
          setModel((prev) => {
            if (!list.some((m) => m.id === prev)) {
              return list[0].id;
            }
            return prev;
          });
        }
      } else {
        throw new Error(result.error || 'Failed to fetch models');
      }
    } catch (e) {
      throw new Error((e as Error)?.message || 'Network error');
    }
  }, []);

  const handleKeyChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const inputValue = e.target.value?.trim();
      setApiKey(inputValue);
      if (!inputValue) return;

      resetSteps();

      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = setTimeout(async () => {
        try {
          setModelsLoading(true);
          const saveResult = await saveAiCredential({
            provider: 'deepseek',
            api_key: inputValue,
            is_default: true,
          });
          if (saveResult.success) {
            await fetchModels();
          } else {
            throw new Error(saveResult.error || 'Failed to save API key');
          }
        } catch (e) {
          setStep1Error((e as Error)?.message || 'Network error');
          setStep1Status('error');
        } finally {
          setModelsLoading(false);
        }
      }, 600);
    },
    [resetSteps, fetchModels],
  );

  const handleSelectModel = useCallback((m: string) => {
    setModel(m);
  }, []);

  const renderStep1 = useCallback(() => {
    const disabled = !apiKey || !model || step1Status === 'verifying' || modelsLoading;
    return (
      <div className="space-y-3 flex flex-col justify-start">
        <Input
          type="password"
          value={apiKey}
          onChange={handleKeyChange}
          loading={modelsLoading}
          placeholder="API Key"
        />
        {models.length > 0 && (
          <div>
            <p className="text-caption tracking-[0.15em] text-on-surface-muted uppercase mb-2">
              Model
            </p>
            <div className="flex flex-wrap gap-1.5">
              {models.map((m) => (
                <div
                  key={m.id}
                  onClick={() => handleSelectModel(m.id)}
                  className={`px-2.5 py-1 rounded-md text-caption border transition-all duration-200 cursor-pointer ${
                    model === m.id
                      ? 'bg-accent-light border-accent-muted text-on-surface'
                      : 'bg-surface-1 border-line-default text-on-surface-tertiary hover:bg-surface-2'
                  }`}
                >
                  {m.id}
                </div>
              ))}
            </div>
          </div>
        )}
        {step1Error && <p className="text-xs text-danger-text">{step1Error}</p>}
        <Button size="small" onClick={startStep1} disabled={disabled}>
          {step1Status === 'verifying' ? 'Verifying...' : 'Verify'}
        </Button>
      </div>
    );
  }, [
    apiKey,
    modelsLoading,
    step1Error,
    step1Status,
    model,
    models,
    handleKeyChange,
    handleSelectModel,
    startStep1,
  ]);

  const renderStep2 = useCallback(() => {
    return (
      <div className="space-y-2">
        {step2Status === 'verifying' && (
          <p className="text-xs text-on-surface-tertiary">Testing connection to DeepSeek API...</p>
        )}
        {step2Status === 'error' && (
          <p className="text-xs text-danger-text">{step2Error || 'Connection failed'}</p>
        )}
      </div>
    );
  }, [step2Status, step2Error]);

  const renderStep3 = useCallback(() => {
    return (
      <div className="space-y-3">
        {balance && (
          <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
            <span className="text-xs text-on-surface-tertiary">Balance</span>
            <span className="text-xs text-on-surface-secondary font-mono">
              {balance.total_balance} {balance.currency}
            </span>
          </div>
        )}
        {step3Status === 'error' && (
          <p className="text-xs text-danger-text">{step3Error || 'Connection failed'}</p>
        )}
      </div>
    );
  }, [balance, step3Status, step3Error]);

  const steps: FlowStepConfig[] = useMemo(
    () => [
      {
        key: 'apiKey',
        title: 'API Key',
        editable: true,
        render: renderStep1,
      },
      {
        key: 'connectivity',
        title: 'Connectivity',
        description: 'Test LLM API inference capability',
        render: renderStep2,
      },
      {
        key: 'account',
        title: 'Account Info',
        description: 'Balance',
        render: renderStep3,
      },
    ],
    [renderStep1, renderStep2, renderStep3],
  );

  const handleBack = useCallback(() => {
    navigate('/setup/bot-type', { replace: true });
  }, [navigate]);

  const handleContinue = useCallback(() => {
    updateWizard({
      llm_provider: 'deepseek',
      llm_model: model,
    });
    advanceStep(WizardStep.SelectExchange);
    startTransition(() => {
      navigate('/setup/exchange', { replace: true });
    });
  }, [model, updateWizard, advanceStep, navigate]);

  const actions = useMemo(() => {
    const disabled = step1Status !== 'done' || step2Status !== 'done' || step3Status !== 'done';
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
  }, [step1Status, step2Status, step3Status, isPending, handleBack, handleContinue]);

  const statuses = useMemo(
    () => ({
      apiKey: step1Status,
      connectivity: step2Status,
      account: step3Status,
    }),
    [step1Status, step2Status, step3Status],
  );

  const summaries = useMemo(() => {
    const summaryMap: Record<string, string | ReactNode> = {};
    if (step1Status === 'done') {
      summaryMap.apiKey = `${apiKey.slice(0, 6)}...${apiKey.slice(-4)} · ${model}`;
    }

    if (step2Status === 'done') {
      summaryMap.connectivity = 'Connected to DeepSeek API';
    } else if (step2Status === 'error') {
      summaryMap.connectivity = 'Connection failed';
    }

    if (step3Status === 'done') {
      if (balance) {
        summaryMap.account = `Balance: ${balance.total_balance} ${balance.currency}`;
      }
    }
    return summaryMap;
  }, [step1Status, step2Status, step3Status, balance, apiKey, model]);

  return (
    <Wizard
      step={WizardStep.ConfigureLlm}
      title="Configure DeepSeek"
      subtitle="Connect your DeepSeek account"
      actions={actions}
    >
      <FlowSteps steps={steps} statuses={statuses} summaries={summaries} />
    </Wizard>
  );
};

export default ConfigureLlm;
