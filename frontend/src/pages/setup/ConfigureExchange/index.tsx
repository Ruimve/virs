import { useState, useCallback, useRef, useMemo, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import { Check, Lock } from '@/components/Icon';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import {
  saveCredential,
  testCredential,
  checkPermissions,
  fetchPositionMode,
} from '../../../service';
import type { PermissionItem, PositionModeResult } from '../../../service';
import { WizardStep } from '../context/WizardContext/consts';
import { Button } from '@/components/Button';
import { Input } from '@/components/Input';
import { FormCard, FormField, InlineBadge, type BadgeState } from '../components';

const KeyIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} className="w-4 h-4">
    <path
      d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

const normalizePemSecret = (raw: string): string => {
  const value = raw.trim();

  const beginIdx = value.search(/-----BEGIN [A-Z ]*PRIVATE KEY-----/);
  const endIdx = value.search(/-----END [A-Z ]*PRIVATE KEY-----/);
  if (beginIdx === -1 || endIdx === -1 || beginIdx >= endIdx) {
    return value;
  }

  const header = value.slice(beginIdx, value.indexOf('-----', beginIdx + 10) + 5);
  const footer = value.slice(endIdx, value.indexOf('-----', endIdx + 8) + 5);

  const bodyStart = value.indexOf(header) + header.length;
  const bodyEnd = value.indexOf(footer);
  const body = value.slice(bodyStart, bodyEnd).trim();

  if (body.includes('\n')) {
    return `${header}\n${body}\n${footer}`;
  }

  const lines: string[] = [];
  for (let i = 0; i < body.length; i += 64) {
    lines.push(body.slice(i, i + 64));
  }
  return `${header}\n${lines.join('\n')}\n${footer}`;
};

type VerifyStage = 'idle' | 'verifying' | 'done' | 'error';

const ConfigureExchange = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.SelectExchange);

  const [apiKey, setApiKey] = useState('');
  const [apiSecret, setApiSecret] = useState('');

  const [connBadgeState, setConnBadgeState] = useState<BadgeState>('idle');
  const [connBadgeText, setConnBadgeText] = useState('');

  const [permStage, setPermStage] = useState<VerifyStage>('idle');
  const [permissions, setPermissions] = useState<PermissionItem[]>([]);

  const [posStage, setPosStage] = useState<VerifyStage>('idle');
  const [, setPosResult] = useState<PositionModeResult | null>(null);
  const [posError, setPosError] = useState('');

  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const resetVerification = useCallback(() => {
    setConnBadgeState('idle');
    setConnBadgeText('');
    setPermStage('idle');
    setPermissions([]);
    setPosStage('idle');
    setPosResult(null);
    setPosError('');
  }, []);

  const checkPositionMode = useCallback(async () => {
    setPosStage('verifying');
    try {
      const result = await fetchPositionMode();
      if (!result.success || !result.data) {
        setPosStage('error');
        setPosError(result.message || 'Failed to query position mode');
        return;
      }

      const { supported, mode } = result.data;
      setPosResult(result.data);
      if (!supported) {
        setPosStage('error');
        setPosError('当前交易所不支持持仓模式查询，请确认账户配置。');
        return;
      }

      if (mode === 'hedge') {
        setPosStage('done');
        return;
      }

      setPosStage('error');
      setPosError(
        '当前为单向持仓模式。请在 Binance APP > 合约 > 设置 > 持仓模式 中切换到双向持仓后重新验证。',
      );
    } catch {
      setPosStage('error');
      setPosError('Network error');
    }
  }, []);

  const checkPerms = useCallback(async () => {
    setPermStage('verifying');
    try {
      const result = await checkPermissions();
      if (result.success && result.data?.permissions) {
        setPermissions(result.data.permissions);
        const allOk = result.data.permissions.every(
          (p) => p.status === 'ok' || p.status === 'warn',
        );
        if (allOk) {
          setPermStage('done');
          await checkPositionMode();
          return;
        }
        setPermStage('error');
        return;
      }

      setPermStage('error');
    } catch {
      setPermStage('error');
    }
  }, [checkPositionMode]);

  const testConn = useCallback(async () => {
    setConnBadgeState('verifying');
    setConnBadgeText('Verifying...');
    try {
      const result = await testCredential();
      if (result.success && result.data?.connected) {
        setConnBadgeState('success');
        setConnBadgeText('Connected');
        await checkPerms();
        return;
      }
      setConnBadgeState('error');
      setConnBadgeText(result.message || result.data?.message || 'Connection failed');
    } catch {
      setConnBadgeState('error');
      setConnBadgeText('Network error');
    }
  }, [checkPerms]);

  const runVerification = useCallback(
    async (key: string, secret: string) => {
      setConnBadgeState('verifying');
      setConnBadgeText('Verifying...');
      setPermStage('idle');
      setPosStage('idle');
      try {
        const res = await saveCredential({
          exchange: 'binance',
          api_key: key,
          api_secret: secret,
          label: 'binance verification',
        });
        if (!res.success) {
          setConnBadgeState('error');
          setConnBadgeText('Failed to save credentials');
          return;
        }
        await testConn();
      } catch {
        setConnBadgeState('error');
        setConnBadgeText('Network error');
      }
    },
    [testConn],
  );

  const handleApiKeyChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const val = e.target.value;
      setApiKey(val);
      resetVerification();
      if (!val || !apiSecret) return;

      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = setTimeout(() => runVerification(val, apiSecret), 600);
    },
    [apiSecret, resetVerification, runVerification],
  );

  const handleApiSecretChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const val = normalizePemSecret(e.target.value);
      setApiSecret(val);
      resetVerification();
      if (!apiKey || !val) return;

      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = setTimeout(() => runVerification(apiKey, val), 600);
    },
    [apiKey, resetVerification, runVerification],
  );

  const handleRetryPositionMode = useCallback(() => {
    checkPositionMode();
  }, [checkPositionMode]);

  const handleBack = useCallback(() => {
    navigate('/setup/llm', { replace: true });
  }, [navigate]);

  const handleContinue = useCallback(() => {
    updateWizard({ exchange: 'binance' });
    advanceStep(WizardStep.ConfigureParams);
    startTransition(() => {
      navigate('/setup/params', { replace: true });
    });
  }, [updateWizard, advanceStep, navigate]);

  const showResults = connBadgeState === 'success' || connBadgeState === 'error';

  const actions = useMemo(() => {
    const disabled = connBadgeState !== 'success' || permStage !== 'done' || posStage !== 'done';
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
  }, [connBadgeState, permStage, posStage, isPending, handleBack, handleContinue]);

  const renderPermIcon = useCallback((status: string) => {
    if (status === 'ok')
      return (
        <span className="text-success-text">
          <Check className="w-3.5 h-3.5" strokeWidth={2.5} />
        </span>
      );
    if (status === 'warn')
      return (
        <span className="text-warning-text">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            className="w-3.5 h-3.5"
          >
            <path
              d="M12 9v4M12 17h.01M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </span>
      );
    return (
      <span className="text-danger-text">
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={2}
          className="w-3.5 h-3.5"
        >
          <path d="M18 6L6 18M6 6l12 12" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </span>
    );
  }, []);

  return (
    <Wizard
      step={WizardStep.SelectExchange}
      title="Connect Binance"
      subtitle="API credentials with read and trade permissions"
      actions={actions}
    >
      <FormCard>
        <FormField label="API Key" required>
          <Input
            mono
            prefix={<KeyIcon />}
            placeholder="API Key"
            value={apiKey}
            onChange={handleApiKeyChange}
          />
        </FormField>
        <FormField
          label="Secret Key"
          required
          badge={<InlineBadge state={connBadgeState} text={connBadgeText} />}
        >
          <Input
            type="password"
            mono
            prefix={<Lock className="w-4 h-4" strokeWidth={2} />}
            placeholder="Secret Key"
            value={apiSecret}
            onChange={handleApiSecretChange}
          />
        </FormField>

        {showResults && (
          <>
            <FormField label="Permissions">
              {permStage === 'verifying' && (
                <p className="text-xs text-on-surface-tertiary">Checking permissions...</p>
              )}
              {permStage === 'idle' && (
                <p className="text-xs text-on-surface-muted">Pending connection verification</p>
              )}
              {permStage === 'error' && (
                <p className="text-xs text-danger-text">Permission check failed</p>
              )}
              {permStage === 'done' &&
                permissions.map((p, i) => (
                  <div
                    key={i}
                    className="flex items-center justify-between py-1.5 first:pt-0 last:pb-0"
                  >
                    <div className="flex items-center gap-2">
                      {renderPermIcon(p.status)}
                      <span className="text-xs text-on-surface-tertiary">{p.label}</span>
                    </div>
                    <span
                      className={`text-caption ${
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
            </FormField>

            <FormField label="Position Mode" noBorder>
              {posStage === 'verifying' && (
                <p className="text-xs text-on-surface-tertiary">Checking position mode...</p>
              )}
              {posStage === 'idle' && (
                <p className="text-xs text-on-surface-muted">Pending permission check</p>
              )}
              {posStage === 'done' && (
                <p className="text-xs text-success-text">Hedge Mode (双向持仓) ✓</p>
              )}
              {posStage === 'error' && (
                <div className="space-y-1.5">
                  <p className="text-xs text-danger-text">{posError}</p>
                  <Button size="small" onClick={handleRetryPositionMode}>
                    重新验证
                  </Button>
                </div>
              )}
            </FormField>
          </>
        )}
      </FormCard>
    </Wizard>
  );
};

export default ConfigureExchange;
