import { useState, useCallback, type ReactNode, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { Wizard } from '../context/WizardContext/Wizard';
import { FlowSteps, type FlowStepConfig, type FlowStepStatus } from '../../../components/FlowStep';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import {
  saveCredential,
  testCredential,
  checkPermissions,
  fetchPositionMode,
} from '../../../service';
import type { PermissionItem } from '../../../service';
import { WizardStep } from '../context/WizardContext/consts';

export type MarketType = 'perpetual' | 'spot';

const MARKET_TYPES: Array<{ id: MarketType; label: string; desc: string }> = [
  { id: 'perpetual', label: 'Perpetual', desc: 'USDT-M futures' },
  { id: 'spot', label: 'Spot', desc: 'Spot trading' },
];

/**
 * 规范化 PEM 格式的 API Secret。
 *
 * 用户从文本编辑器或浏览器复制 PEM 时，换行符 `\n` 经常会被替换成空格，
 * 导致后端 `from_pkcs8_pem` 解析失败。此函数检测 PEM 格式并还原换行符。
 *
 * 处理逻辑：
 * - 检测 `-----BEGIN ... PRIVATE KEY-----` 和 `-----END ... PRIVATE KEY-----` 标记
 * - header / base64 内容 / footer 之间的空格替换为 `\n`
 * - base64 内容内部如果是单行（无空格），保持原样（合法的单行 PEM）
 * - 非 PEM 格式（HMAC secret、base64 seed）原样返回
 */
const normalizePemSecret = (raw: string): string => {
  const value = raw.trim();
  // 必须同时包含 BEGIN 和 END 标记才视为 PEM
  const beginIdx = value.search(/-----BEGIN [A-Z ]*PRIVATE KEY-----/);
  const endIdx = value.search(/-----END [A-Z ]*PRIVATE KEY-----/);
  if (beginIdx === -1 || endIdx === -1 || beginIdx >= endIdx) {
    return value;
  }

  const header = value.slice(beginIdx, value.indexOf('-----', beginIdx + 10) + 5);
  const footer = value.slice(endIdx, value.indexOf('-----', endIdx + 8) + 5);
  // header 和 footer 之间的内容（base64 体）
  const bodyStart = value.indexOf(header) + header.length;
  const bodyEnd = value.indexOf(footer);
  const body = value.slice(bodyStart, bodyEnd).trim();

  // 如果 body 本身包含换行，保留原样
  if (body.includes('\n')) {
    return `${header}\n${body}\n${footer}`;
  }
  // 单行 body：尝试按 64 字符宽度重新分行（OpenSSL 默认输出格式）
  const lines: string[] = [];
  for (let i = 0; i < body.length; i += 64) {
    lines.push(body.slice(i, i + 64));
  }
  return `${header}\n${lines.join('\n')}\n${footer}`;
};

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

  // Step 4: Position mode (perpetual only — spot auto-skips)
  const [step4Status, setStep4Status] = useState<FlowStepStatus>('pending');
  const [step4Error, setStep4Error] = useState<string | null>(null);

  const resetSteps = useCallback(() => {
    setStep1Status('active');
    setStep1Error(null);
    setStep2Status('pending');
    setStep2Error(null);
    setStep3Status('pending');
    setStep3Error(null);
    setStep4Status('pending');
    setStep4Error(null);
  }, []);

  // Step 4: Check position mode (perpetual only — spot auto-skips)
  const startStep4 = useCallback(async () => {
    setStep4Status('verifying');
    try {
      const result = await fetchPositionMode();
      if (!result.success || !result.data) {
        setStep4Status('error');
        setStep4Error(result.error || 'Failed to query position mode');
        return;
      }

      const { supported, mode } = result.data;
      // Spot exchanges don't support position mode — skip this step.
      if (!supported) {
        setStep4Status('done');
        return;
      }

      if (mode === 'hedge') {
        setStep4Status('done');
        return;
      }

      // OneWay — block: user must switch to Hedge mode in Binance APP.
      setStep4Status('error');
      setStep4Error(
        '当前为单向持仓模式。请在 Binance APP > 合约 > 设置 > 持仓模式 中切换到双向持仓后重新验证。',
      );
    } catch {
      setStep4Status('error');
      setStep4Error('Network error');
    }
  }, []);

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
        if (allOk) {
          setStep3Status('done');
          startStep4();
          return;
        }
        setStep3Status('active');
        return;
      }

      setStep3Status('error');
      setStep3Error(result.error || 'Permission check failed');
    } catch {
      setStep3Status('error');
      setStep3Error('Network error');
    }
  }, [startStep4]);

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

  const handleApiKeyChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setApiKey(e.target.value);
      resetSteps();
    },
    [resetSteps],
  );

  const handleApiSecretChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setApiSecret(normalizePemSecret(e.target.value));
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
          onChange={handleApiKeyChange}
          className="w-full px-4 py-2.5 bg-surface-2 border border-line-strong rounded-lg text-sm text-on-base placeholder-placeholder focus:outline-none focus:border-accent transition-all duration-200"
          placeholder="API Key"
        />
        <input
          type="password"
          value={apiSecret}
          onChange={handleApiSecretChange}
          className="w-full px-4 py-2.5 bg-surface-2 border border-line-strong rounded-lg text-sm text-on-base placeholder-placeholder focus:outline-none focus:border-accent transition-all duration-200"
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
                    ? 'bg-accent-light border-accent-muted text-on-base'
                    : 'bg-surface-1 border-line-default text-on-surface-tertiary hover:bg-surface-2'
                }`}
              >
                <p className="text-xs font-medium">{mt.label}</p>
                <p className="text-[10px] text-on-surface-muted mt-0.5">{mt.desc}</p>
              </button>
            ))}
          </div>
        </div>
        {step1Status === 'error' && <p className="text-[12px] text-danger-text">{step1Error}</p>}
        <button
          onClick={startStep1}
          disabled={disabled}
          className="px-4 py-2 text-[12px] bg-accent-muted border border-accent-muted rounded-lg text-accent hover:bg-accent-muted disabled:opacity-30 transition-all duration-200"
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
    handleApiKeyChange,
    handleApiSecretChange,
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
          <p className="text-[12px] text-danger-text">{step2Error || 'Connection failed'}</p>
        )}
      </div>
    );
  }, [step2Status, step2Error]);

  const renderStatusIcon = useCallback((status: string) => {
    if (status === 'ok') return <span className="text-success-text">&#10003;</span>;
    if (status === 'warn') return <span className="text-warning-text">&#9888;</span>;
    return <span className="text-danger-text">&#10007;</span>;
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
                    ? 'text-warning-text/60'
                    : 'text-danger-text/60'
              }`}
            >
              {p.detail}
            </span>
          </div>
        ))}
        {step3Status === 'error' && (
          <p className="text-[12px] text-danger-text">{step3Error || 'Connection failed'}</p>
        )}
      </div>
    );
  }, [permissions, step3Status, step3Error, renderStatusIcon]);

  const renderStep4 = useCallback(() => {
    return (
      <div className="space-y-2">
        {step4Status === 'verifying' && (
          <p className="text-[12px] text-on-surface-tertiary">Checking position mode...</p>
        )}
        {step4Status === 'done' && (
          <p className="text-[12px] text-success-text">双向持仓模式 (Hedge Mode) ✓</p>
        )}
        {step4Status === 'error' && (
          <div className="space-y-1">
            <p className="text-[12px] text-danger-text">
              {step4Error || 'Position mode check failed'}
            </p>
            <button
              onClick={startStep4}
              className="px-3 py-1.5 text-[11px] bg-surface-2 border border-line-strong rounded-lg text-on-surface-secondary hover:bg-surface-1 transition-all duration-200"
            >
              重新验证
            </button>
          </div>
        )}
      </div>
    );
  }, [step4Status, step4Error, startStep4]);

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
      {
        key: 'position-mode',
        title: 'Position Mode',
        description: 'Verify hedge mode is enabled (perpetual only)',
        render: renderStep4,
      },
    ],
    [renderStep1, renderStep2, renderStep3, renderStep4],
  );

  const actions = useMemo(() => {
    const disabled =
      step1Status !== 'done' ||
      step2Status !== 'done' ||
      step3Status !== 'done' ||
      step4Status !== 'done';
    return (
      <>
        <button
          onClick={() => navigate('/setup/llm', { replace: true })}
          className="w-full sm:w-auto sm:px-5 py-2.5 text-sm text-on-surface-tertiary hover:text-on-surface-secondary rounded-xl transition-colors duration-200"
        >
          Back
        </button>
        <button
          onClick={() => {
            updateWizard({ exchange: 'binance', market_type: marketType });
            advanceStep(WizardStep.ConfigureParams);
            navigate('/setup/params', { replace: true });
          }}
          disabled={disabled}
          className="w-full sm:w-auto sm:px-6 py-2.5 bg-accent/80 hover:bg-accent-hover text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
        >
          Continue
        </button>
      </>
    );
  }, [
    step1Status,
    step2Status,
    step3Status,
    step4Status,
    marketType,
    updateWizard,
    advanceStep,
    navigate,
  ]);

  const statuses = useMemo(() => {
    return {
      credentials: step1Status,
      connectivity: step2Status,
      permissions: step3Status,
      'position-mode': step4Status,
    };
  }, [step1Status, step2Status, step3Status, step4Status]);

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

    if (step4Status === 'done') {
      summaryMap['position-mode'] = 'Hedge mode ✓';
    } else if (step4Status === 'error') {
      summaryMap['position-mode'] = 'OneWay — switch to Hedge';
    }

    return summaryMap;
  }, [apiKey, step1Status, step2Status, step3Status, step4Status]);

  return (
    <Wizard
      step={WizardStep.SelectExchange}
      title="Connect Binance"
      subtitle="Provide your API credentials"
      actions={actions}
    >
      <FlowSteps steps={steps} statuses={statuses} summaries={summaries} />
    </Wizard>
  );
};

export default ConfigureExchange;
