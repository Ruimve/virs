import { useState, useCallback, useRef, type ReactNode, useMemo } from 'react';
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
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import { WizardStep } from '../context/WizardContext/consts';

const ConfigureLlm = () => {
  const navigate = useNavigate();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.ConfigureLlm);

  // Step 1: API Key + Model
  const [step1Status, setStep1Status] = useState<FlowStepStatus>('active');
  const [step1Error, setStep1Error] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState('');
  const [models, setModels] = useState<DeepSeekModel[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [model, setModel] = useState(wizard.llm_model);

  // Step 2: Connectivity
  const [step2Status, setStep2Status] = useState<FlowStepStatus>('pending');
  const [step2Error, setStep2Error] = useState<string | null>(null);

  // Step 3: Account Info (balance)
  const [step3Status, setStep3Status] = useState<FlowStepStatus>('pending');
  const [step3Error, setStep3Error] = useState<string | null>(null);
  const [balance, setBalance] = useState<BalanceInfo | null>(null);

  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleContinue = () => {
    updateWizard({
      llm_provider: 'deepseek',
      llm_model: model,
    });
    advanceStep(WizardStep.SelectExchange);
    navigate('/setup/exchange', { replace: true });
  };

  // 重置步骤状态
  const resetSteps = useCallback(() => {
    setStep1Status('active');
    setStep1Error(null);
    setStep2Status('pending');
    setStep2Error(null);
    setStep3Status('pending');
    setStep3Error(null);
  }, []);

  // Fetch balance via backend proxy (uses saved credentials)
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

  // Test connectivity via backend (uses saved credentials)
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

  // Fetch models via backend proxy (after save)
  const fetchModels = useCallback(async () => {
    setModelsLoading(true);
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
        setStep1Error(result.error || 'Failed to fetch models');
        setStep1Status('error');
      }
    } catch {
      setStep1Error('Network error');
      setStep1Status('error');
    } finally {
      setModelsLoading(false);
    }
  }, []);

  const handleKeyInput = useCallback(
    (e: React.InputEvent<HTMLInputElement>) => {
      const inputValue = e.currentTarget.value?.trim();
      setApiKey(inputValue);
      if (!inputValue) return;

      resetSteps();

      // Debounce: save then fetch models
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = setTimeout(async () => {
        const saveResult = await saveAiCredential({
          provider: 'deepseek',
          api_key: inputValue,
          is_default: true,
        });
        if (saveResult.success) {
          fetchModels();
        } else {
          setStep1Error(saveResult.error || 'Failed to save API key');
          setStep1Status('error');
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
      <div className="space-y-3">
        <div className="relative">
          <input
            type="password"
            value={apiKey}
            onInput={handleKeyInput}
            disabled={modelsLoading}
            className={`w-full px-4 py-2.5 bg-surface-2 border rounded-lg text-sm text-on-base placeholder-placeholder focus:outline-none transition-all duration-200 ${
              modelsLoading
                ? 'border-indigo-500/30 opacity-60'
                : 'border-line-strong focus:border-indigo-500/40'
            }`}
            placeholder="API Key"
          />
          {modelsLoading && (
            <div className="absolute right-3 top-1/2 -translate-y-1/2">
              <svg className="animate-spin w-4 h-4 text-indigo-400" fill="none" viewBox="0 0 24 24">
                <circle
                  className="opacity-25"
                  cx="12"
                  cy="12"
                  r="10"
                  stroke="currentColor"
                  strokeWidth="4"
                />
                <path
                  className="opacity-75"
                  fill="currentColor"
                  d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                />
              </svg>
            </div>
          )}
        </div>
        {models.length > 0 && (
          <div>
            <p className="text-[11px] tracking-[0.15em] text-on-surface-muted uppercase mb-2">
              Model
            </p>
            <div className="flex flex-wrap gap-1.5">
              {models.map((m) => (
                <button
                  key={m.id}
                  onClick={() => handleSelectModel(m.id)}
                  className={`px-2.5 py-1 rounded-md text-[11px] border transition-all duration-200 ${
                    model === m.id
                      ? 'bg-indigo-500/15 border-indigo-500/30 text-on-surface'
                      : 'bg-surface-1 border-line-default text-on-surface-tertiary hover:bg-surface-2'
                  }`}
                >
                  {m.id}
                </button>
              ))}
            </div>
          </div>
        )}
        {step1Error && <p className="text-[12px] text-red-400">{step1Error}</p>}
        <button
          onClick={startStep1}
          disabled={disabled}
          className="px-4 py-2 text-[12px] bg-indigo-500/20 border border-indigo-500/30 rounded-lg text-indigo-300 hover:bg-indigo-500/30 disabled:opacity-30 transition-all duration-200"
        >
          {step1Status === 'verifying' ? 'Verifying...' : 'Verify'}
        </button>
      </div>
    );
  }, [
    apiKey,
    modelsLoading,
    step1Error,
    step1Status,
    model,
    models,
    handleKeyInput,
    handleSelectModel,
    startStep1,
  ]);

  const renderStep2 = useCallback(() => {
    return (
      <div className="space-y-2">
        {step2Status === 'verifying' && (
          <p className="text-[12px] text-on-surface-tertiary">
            Testing connection to DeepSeek API...
          </p>
        )}
        {step2Status === 'error' && (
          <p className="text-[12px] text-red-400">{step2Error || 'Connection failed'}</p>
        )}
      </div>
    );
  }, [step2Status, step2Error]);

  const renderStep3 = useCallback(() => {
    return (
      <div className="space-y-3">
        {balance && (
          <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
            <span className="text-[12px] text-on-surface-tertiary">Balance</span>
            <span className="text-[12px] text-on-surface-secondary font-mono">
              {balance.total_balance} {balance.currency}
            </span>
          </div>
        )}
        {step3Status === 'error' && (
          <p className="text-[12px] text-red-400">{step3Error || 'Connection failed'}</p>
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

  const canContinue = step2Status === 'done' && step3Status === 'done';

  return (
    <Wizard
      step={WizardStep.ConfigureLlm}
      title="Configure DeepSeek"
      subtitle="Connect your DeepSeek account"
      actions={
        <>
          <button
            onClick={() => navigate('/setup/bot-type', { replace: true })}
            className="w-full sm:w-auto sm:px-5 py-2.5 text-sm text-on-surface-tertiary hover:text-on-surface-secondary rounded-xl transition-colors duration-200"
          >
            Back
          </button>
          <button
            onClick={handleContinue}
            disabled={!canContinue}
            className="w-full sm:w-auto sm:px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
          >
            Continue
          </button>
        </>
      }
    >
      <FlowSteps steps={steps} statuses={statuses} summaries={summaries} />
    </Wizard>
  );
};

export default ConfigureLlm;
