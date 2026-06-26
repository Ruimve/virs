import { useState, useCallback, type ReactNode, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { Wizard } from '../context/WizardContext/Wizard';
import { FlowSteps, type FlowStepConfig, type FlowStepStatus } from '../../../components/FlowStep';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import { saveCredential, testCredential, checkPermissions } from '../../../service';
import type { PermissionItem } from '../../../service';
import { WizardStep } from '../context/WizardContext/consts';

export type MarketType = 'perpetual' | 'spot';

const MARKET_TYPES: Array<{ id: MarketType; label: string; desc: string }> = [
  { id: 'perpetual', label: 'Perpetual', desc: 'USDT-M futures' },
  { id: 'spot', label: 'Spot', desc: 'Spot trading' },
];

const ConfigureExchange = () => {
  const navigate = useNavigate();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.SelectExchange);

  // Step 1: API credentials
  const [step1Status, setStep1Status] = useState<FlowStepStatus>('active');
  const [step1Error, setStep1Error] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState('');
  const [apiSecret, setApiSecret] = useState('');
  const [marketType, setMarketType] = useState<MarketType>(wizard.market_type);

  // Step 2: Connectivity + Permissions
  const [step2Status, setStep2Status] = useState<FlowStepStatus>('pending');
  const [step2Error, setStep2Error] = useState<string | null>(null);

  // Step 3: Permissions
  const [step3Status, setStep3Status] = useState<FlowStepStatus>('pending');
  const [step3Error, setStep3Error] = useState<string | null>(null);
  const [permissions, setPermissions] = useState<PermissionItem[]>([]);

  const resetSteps = useCallback(() => {
    setStep1Status('active');
    setStep1Error(null);
    setStep2Status('pending');
    setStep2Error(null);
    setStep3Status('pending');
    setStep2Error(null);
  }, []);

  const handleContinue = () => {
    updateWizard({ exchange: 'binance', market_type: marketType });
    advanceStep(WizardStep.ConfigureParams);
    navigate('/setup/params', { replace: true });
  };

  // Check permissions via apiRestrictions
  const startStep3 = useCallback(async () => {
    setStep3Status('verifying');
    try {
      const result = await checkPermissions();
      if (result.success && result.data?.permissions) {
        setPermissions(result.data.permissions);
        const allOk = result.data.permissions.every(
          (p) => p.status === 'ok' || p.status === 'warn',
        );
        setStep3Status(allOk ? 'done' : 'active');
        return;
      }

      setStep3Status('error');
      setStep3Error(result.error || 'Permission check failed');
    } catch {
      setStep2Status('error');
      setStep2Error('Network error');
    }
  }, []);

  // Test connectivity only (ping) — uses saved credentials from registry
  const startStep2 = useCallback(async () => {
    setStep2Status('verifying');
    try {
      const result = await testCredential();
      if (result.success && result.data?.connected) {
        setStep2Status('done');
        startStep3();
        return;
      }

      setStep2Status('error');
      setStep2Error(result.error || result.data?.message || 'Connection failed');
    } catch {
      setStep2Status('error');
      setStep2Error('Network error');
    }
  }, [startStep3]);

  // Verify: Step 1 save → done, then auto-start Step 2
  const startStep1 = useCallback(async () => {
    setStep1Status('verifying');
    setStep1Error('');

    try {
      const res = await saveCredential({
        exchange: 'binance',
        api_key: apiKey,
        api_secret: apiSecret,
        market_type: marketType,
        label: 'binance verification',
      });
      if (res.success) {
        setStep1Status('done');
        startStep2();
        return;
      }

      setStep1Status('error');
      setStep1Error('Failed to save credentials');
    } catch {
      setStep1Status('error');
      setStep1Error('Network error');
    }
  }, [apiKey, apiSecret, marketType, startStep2]);

  const handleApiKeyInput = useCallback(
    (e: React.InputEvent<HTMLInputElement>) => {
      setApiKey(e.currentTarget.value);
      resetSteps();
    },
    [resetSteps],
  );

  const handleApiSecretInput = useCallback(
    (e: React.InputEvent<HTMLInputElement>) => {
      setApiSecret(e.currentTarget.value);
      resetSteps();
    },
    [resetSteps],
  );

  const handleSelectMarketType = useCallback((mt: MarketType) => {
    setMarketType(mt);
  }, []);

  const renderStep1 = useCallback(() => {
    const disabled = !apiKey || !apiSecret || step1Status === 'verifying';
    return (
      <div className="space-y-3">
        <input
          type="text"
          value={apiKey}
          onInput={handleApiKeyInput}
          className="w-full px-4 py-2.5 bg-surface-2 border border-line-strong rounded-lg text-sm text-on-base placeholder-placeholder focus:outline-none focus:border-indigo-500/40 transition-all duration-200"
          placeholder="API Key"
        />
        <input
          type="password"
          value={apiSecret}
          onInput={handleApiSecretInput}
          className="w-full px-4 py-2.5 bg-surface-2 border border-line-strong rounded-lg text-sm text-on-base placeholder-placeholder focus:outline-none focus:border-indigo-500/40 transition-all duration-200"
          placeholder="API Secret"
        />
        <div>
          <p className="text-[11px] tracking-[0.15em] text-on-surface-muted uppercase mb-2">
            Market Type
          </p>
          <div className="grid grid-cols-2 gap-2">
            {MARKET_TYPES.map((mt) => (
              <button
                key={mt.id}
                onClick={() => handleSelectMarketType(mt.id)}
                className={`p-2.5 rounded-lg border text-center transition-all duration-200 ${
                  marketType === mt.id
                    ? 'bg-indigo-500/10 border-indigo-500/30 text-on-base'
                    : 'bg-surface-1 border-line-default text-on-surface-tertiary hover:bg-surface-2'
                }`}
              >
                <p className="text-xs font-medium">{mt.label}</p>
                <p className="text-[10px] text-on-surface-muted mt-0.5">{mt.desc}</p>
              </button>
            ))}
          </div>
        </div>
        {step1Status === 'error' && <p className="text-[12px] text-red-400">{step1Error}</p>}
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
    apiSecret,
    marketType,
    step1Status,
    step1Error,
    handleApiKeyInput,
    handleApiSecretInput,
    handleSelectMarketType,
    startStep1,
  ]);

  const renderStep2 = useCallback(() => {
    return (
      <div className="space-y-2">
        {step2Status === 'verifying' && (
          <p className="text-[12px] text-on-surface-tertiary">Testing connection to Binance...</p>
        )}
        {step2Status === 'error' && (
          <p className="text-[12px] text-red-400">{step2Error || 'Connection failed'}</p>
        )}
      </div>
    );
  }, [step2Status, step2Error]);

  const renderStatusIcon = useCallback((status: string) => {
    if (status === 'ok') return <span className="text-emerald-400">&#10003;</span>;
    if (status === 'warn') return <span className="text-amber-400">&#9888;</span>;
    return <span className="text-red-400">&#10007;</span>;
  }, []);

  const renderStep3 = useCallback(() => {
    return (
      <div className="space-y-1.5">
        {permissions.map((p, i) => (
          <div
            key={i}
            className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg"
          >
            <div className="flex items-center gap-2">
              <span className="text-[12px]">{renderStatusIcon(p.status)}</span>
              <span className="text-[12px] text-on-surface-tertiary">{p.label}</span>
            </div>
            <span
              className={`text-[11px] ${
                p.status === 'ok'
                  ? 'text-on-surface-muted'
                  : p.status === 'warn'
                    ? 'text-amber-400/60'
                    : 'text-red-400/60'
              }`}
            >
              {p.detail}
            </span>
          </div>
        ))}
        {step3Status === 'error' && (
          <p className="text-[12px] text-red-400">{step3Error || 'Connection failed'}</p>
        )}
      </div>
    );
  }, [permissions, step3Status, step3Error, renderStatusIcon]);

  const steps: FlowStepConfig[] = useMemo(
    () => [
      {
        key: 'credentials',
        title: 'API Credentials',
        render: renderStep1,
      },
      {
        key: 'connectivity',
        title: 'Connectivity',
        description: 'Ping exchange server to verify reachability',
        render: renderStep2,
      },
      {
        key: 'permissions',
        title: 'Permissions',
        description: 'Check API key permissions and restrictions',
        render: renderStep3,
      },
    ],
    [renderStep1, renderStep2, renderStep3],
  );

  const statuses = useMemo(() => {
    return {
      credentials: step1Status,
      connectivity: step2Status,
      permissions: step3Status,
    };
  }, [step1Status, step2Status, step3Status]);

  const summaries = useMemo(() => {
    const summaryMap: Record<string, string | ReactNode> = {};
    if (step1Status === 'done') {
      summaryMap.credentials = `${apiKey.slice(0, 6)}...${apiKey.slice(-4)}`;
    }

    if (step2Status === 'done') {
      summaryMap.connectivity = 'Connected to Binance';
    } else if (step2Status === 'error') {
      summaryMap.connectivity = 'Connection failed';
    }

    if (step3Status === 'done') {
      summaryMap.permissions = 'All checks passed';
    }

    return summaryMap;
  }, [apiKey, step1Status, step2Status, step3Status]);

  const canContinue = step2Status === 'done' && step3Status === 'done';

  return (
    <Wizard
      step={WizardStep.SelectExchange}
      title="Connect Binance"
      subtitle="Provide your API credentials"
      actions={
        <>
          <button
            onClick={() => navigate('/setup/llm', { replace: true })}
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

export default ConfigureExchange;
