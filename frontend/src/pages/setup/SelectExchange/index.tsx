import { useState, useCallback, type ReactNode } from 'react';
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

const SelectExchange = () => {
  const navigate = useNavigate();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.SelectExchange);

  // Step 1: API credentials
  const [apiKey, setApiKey] = useState('');
  const [apiSecret, setApiSecret] = useState('');
  const [selectedMarket, setSelectedMarket] = useState<MarketType>(
    wizard.market_type || 'perpetual',
  );
  const [step1Status, setStep1Status] = useState<FlowStepStatus>('active');
  const [error, setError] = useState('');

  // Step 2: Connectivity + Permissions
  const [step2Status, setStep2Status] = useState<FlowStepStatus>('pending');

  // Step 3: Permissions
  const [permissions, setPermissions] = useState<PermissionItem[]>([]);
  const [step3Status, setStep3Status] = useState<FlowStepStatus>('pending');

  const statuses = {
    credentials: step1Status,
    connectivity: step2Status,
    permissions: step3Status,
  };

  const summaries: Record<string, string | ReactNode> = {};
  if (step1Status === 'done') summaries.credentials = `${apiKey.slice(0, 6)}...${apiKey.slice(-4)}`;
  if (step2Status === 'done') summaries.connectivity = 'Connected to Binance';
  else if (step2Status === 'error') summaries.connectivity = 'Connection failed';
  if (step3Status === 'done') summaries.permissions = 'All checks passed';

  const statusIcon = (status: string) => {
    if (status === 'ok') return <span className="text-emerald-400">&#10003;</span>;
    if (status === 'warn') return <span className="text-amber-400">&#9888;</span>;
    return <span className="text-red-400">&#10007;</span>;
  };

  const resetDownstream = () => {
    if (step2Status !== 'pending') setStep2Status('pending');
    if (step3Status !== 'pending') setStep3Status('pending');
  };

  // Check permissions via apiRestrictions
  const doCheckPermissions = useCallback(async () => {
    setStep3Status('verifying');
    try {
      const result = await checkPermissions();
      if (!result.success || !result.data?.permissions) {
        setError(result.error || 'Permission check failed');
        setStep3Status('error');
        return;
      }
      setPermissions(result.data.permissions);
      const allOk = result.data.permissions.every((p) => p.status === 'ok' || p.status === 'warn');
      setStep3Status(allOk ? 'done' : 'active');
    } catch {
      setError('Permission check failed');
      setStep3Status('error');
    }
  }, []);

  // Test connectivity only (ping) — uses saved credentials from registry
  const doTestConnectivity = useCallback(async () => {
    setStep2Status('verifying');
    try {
      const result = await testCredential();
      if (!result.success || !result.data?.connected) {
        setError(result.data?.message || result.error || 'Connection failed');
        setStep2Status('error');
        return;
      }
      setStep2Status('done');
      doCheckPermissions();
    } catch {
      setError('Connection test failed');
      setStep2Status('error');
    }
  }, [doCheckPermissions]);

  const steps: FlowStepConfig[] = [
    {
      key: 'credentials',
      title: 'API Credentials',
      render: () => (
        <div className="space-y-3">
          <input
            type="text"
            value={apiKey}
            onInput={(e) => {
              setApiKey(e.currentTarget.value);
              setError('');
              if (step1Status === 'error') setStep1Status('active');
              resetDownstream();
            }}
            className="w-full px-4 py-2.5 bg-surface-2 border border-line-strong rounded-lg text-sm text-on-base placeholder-placeholder focus:outline-none focus:border-indigo-500/40 transition-all duration-200"
            placeholder="API Key"
          />
          <input
            type="password"
            value={apiSecret}
            onInput={(e) => {
              setApiSecret(e.currentTarget.value);
              setError('');
              if (step1Status === 'error') setStep1Status('active');
              resetDownstream();
            }}
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
                  onClick={() => {
                    setSelectedMarket(mt.id);
                    resetDownstream();
                  }}
                  className={`p-2.5 rounded-lg border text-center transition-all duration-200 ${
                    selectedMarket === mt.id
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
          {error && <p className="text-[12px] text-red-400">{error}</p>}
          <button
            onClick={verifyCredentials}
            disabled={!apiKey.trim() || !apiSecret.trim() || step1Status === 'verifying'}
            className="px-4 py-2 text-[12px] bg-indigo-500/20 border border-indigo-500/30 rounded-lg text-indigo-300 hover:bg-indigo-500/30 disabled:opacity-30 transition-all duration-200"
          >
            {step1Status === 'verifying' ? 'Verifying...' : 'Verify'}
          </button>
        </div>
      ),
    },
    {
      key: 'connectivity',
      title: 'Connectivity',
      description: 'Ping exchange server to verify reachability',
      render: () => (
        <div className="space-y-2">
          {step2Status === 'verifying' && (
            <p className="text-[12px] text-on-surface-tertiary">Testing connection to Binance...</p>
          )}
          {step2Status === 'error' && (
            <p className="text-[12px] text-red-400">{error || 'Connection failed'}</p>
          )}
        </div>
      ),
    },
    {
      key: 'permissions',
      title: 'Permissions',
      description: 'Check API key permissions and restrictions',
      render: () => (
        <div className="space-y-1.5">
          {permissions.map((p, i) => (
            <div
              key={i}
              className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg"
            >
              <div className="flex items-center gap-2">
                <span className="text-[12px]">{statusIcon(p.status)}</span>
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
        </div>
      ),
    },
  ];

  // Verify: Step 1 save → done, then auto-start Step 2
  const verifyCredentials = async () => {
    const key = apiKey.trim();
    const secret = apiSecret.trim();
    if (!key || !secret) return;

    setStep1Status('verifying');
    setError('');

    try {
      const result = await saveCredential({
        exchange: 'binance',
        api_key: key,
        api_secret: secret,
        market_type: selectedMarket,
        label: 'binance verification',
      });
      if (!result.success) {
        throw new Error(result.error || 'Failed to save credentials');
      }
      setStep1Status('done');
      doTestConnectivity();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save credentials');
      setStep1Status('error');
    }
  };

  const handleContinue = () => {
    updateWizard({ exchange: 'binance', market_type: selectedMarket });
    advanceStep(WizardStep.ConfigureParams);
    navigate('/setup/params', { replace: true });
  };

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

export default SelectExchange;
