import { useState, useCallback, useMemo, useTransition, useEffect, useDeferredValue } from 'react';
import { useNavigate } from 'react-router-dom';
import { Key, Lock } from '@/components/Icon';
import { Button } from '@/components/Button';
import { Input } from '@/components/Input';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import { WizardStep } from '../context/WizardContext/consts';
import { FormCard, FormField, InlineBadge, type Badge } from '../components';
import { checkApiKey, normalizePemSecret } from './utils';
import { Permission } from './Validation/Permission';
import { PositionMode } from './Validation/PositionMode';

const ConfigureExchange = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.SelectExchange);

  const [apiKey, setApiKey] = useState('');
  const deferredApiKey = useDeferredValue(apiKey);
  const [apiSecret, setApiSecret] = useState('');
  const deferredApiSecret = useDeferredValue(apiSecret);

  const [badge, setBadge] = useState<Badge>({ state: 'idle', text: '' });

  const [connection, setConnection] = useState<boolean>(false);
  const [permission, setPermission] = useState<boolean>(false);
  const [positionMode, setPositionMode] = useState<boolean>(false);

  useEffect(() => {
    if (!deferredApiKey || !deferredApiSecret) return;

    setConnection(false);
    setPermission(false);
    setPositionMode(false);
    setBadge({ state: 'verifying', text: 'Verifying...' });

    const { controllers, check } = checkApiKey(deferredApiKey, deferredApiSecret);
    check().then((res) => {
      /** 处理取消请求 */
      if (!res) return;

      /** 处理成功响应 */
      if (res.success) {
        setConnection(true);
        setBadge({ state: 'success', text: res.message });
      } else {
        setBadge({ state: 'error', text: res.message });
      }
    });

    return () => {
      controllers.forEach((controller) => controller.abort());
    };
  }, [deferredApiKey, deferredApiSecret]);

  const handleApiKeyChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setApiKey(val);
    setBadge({ state: 'idle', text: '' });
  }, []);

  const handleApiSecretChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const val = normalizePemSecret(e.target.value);
    setApiSecret(val);
    setBadge({ state: 'idle', text: '' });
  }, []);

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

  const actions = useMemo(() => {
    const disabled = badge.state !== 'success' || !permission || !positionMode;
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
  }, [badge.state, permission, positionMode, isPending, handleBack, handleContinue]);

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
            prefix={<Key width={16} height={16} strokeWidth={2} />}
            placeholder="API Key"
            value={apiKey}
            onChange={handleApiKeyChange}
          />
        </FormField>
        <FormField label="Secret Key" required badge={<InlineBadge badge={badge} />}>
          <Input
            type="password"
            mono
            prefix={<Lock className="w-4 h-4" strokeWidth={2} />}
            placeholder="Secret Key"
            value={apiSecret}
            onChange={handleApiSecretChange}
          />
        </FormField>
        {connection && (
          <>
            <Permission onCheck={setPermission} />
            {permission && <PositionMode onCheck={setPositionMode} />}
          </>
        )}
      </FormCard>
    </Wizard>
  );
};

export default ConfigureExchange;
