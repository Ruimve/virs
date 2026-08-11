import { useState, useCallback, useMemo, useTransition, useEffect, useDeferredValue } from 'react';
import { useNavigate } from 'react-router-dom';
import { saveAiCredential, type DeepSeekModel } from '@/service';
import { Button } from '@/components/Button';
import { Input } from '@/components/Input';
import { Lock } from '@/components/Icon';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import { WizardStep } from '../context/WizardContext/define';
import {
  FormCard,
  FormField,
  FormSelect,
  InlineBadge,
  HelperLink,
  type Badge,
} from '../components';
import { checkApiKey } from './utils';

const ConfigureLlm = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { wizard, updateWizard, advanceStep } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.ConfigureLlm);

  const [apiKey, setApiKey] = useState('');
  const deferredApiKey = useDeferredValue(apiKey);

  const [models, setModels] = useState<DeepSeekModel[]>([]);
  const [model, setModel] = useState(wizard.llm_model);

  const [badge, setBadge] = useState<Badge>({
    state: 'idle',
    text: '',
  });

  useEffect(() => {
    if (!deferredApiKey) return;

    setBadge({ state: 'verifying', text: 'Verifying...' });

    const { controllers, check } = checkApiKey(deferredApiKey);

    check().then((res) => {
      /** 处理取消请求 */
      if (!res) return;

      /** 处理成功响应 */
      if (res.success) {
        setBadge({ state: 'success', text: res.message });
        setModels(res.models);
        if (res.models.length > 0) setModel(res.models[0].id);
      } else {
        setBadge({ state: 'error', text: res.message });
      }
    });

    return () => {
      controllers.forEach((controller) => controller.abort());
    };
  }, [deferredApiKey]);

  const handleKeyChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setApiKey(value);
    setModels([]);
    setBadge({ state: 'idle', text: '' });
  }, []);

  const handleModelChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      const mdl = e.target.value;
      setModel(mdl);
      if (apiKey && mdl) {
        saveAiCredential({
          provider: 'deepseek',
          api_key: apiKey,
          model: mdl,
          is_default: true,
        });
      }
    },
    [apiKey],
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
    const disabled = !apiKey || !model || badge.state !== 'success';
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
  }, [apiKey, model, badge.state, isPending, handleBack, handleContinue]);

  return (
    <Wizard
      step={WizardStep.ConfigureLlm}
      title="Connect DeepSeek"
      subtitle="Enter your API key to power AI trading decisions"
      actions={actions}
    >
      <FormCard>
        <FormField label="API Key" required badge={<InlineBadge badge={badge} />}>
          <Input
            type="password"
            mono
            prefix={<Lock className="w-4 h-4" strokeWidth={2} />}
            placeholder="sk-..."
            value={apiKey}
            onChange={handleKeyChange}
          />
        </FormField>
        <FormField label="Model" required>
          <FormSelect value={model} onChange={handleModelChange} disabled={models.length === 0}>
            {models.length === 0 && <option value="">Select after entering API key</option>}
            {models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.id}
              </option>
            ))}
          </FormSelect>
        </FormField>
      </FormCard>
      <HelperLink href="https://platform.deepseek.com/api_keys">
        Get your DeepSeek API Key
      </HelperLink>
    </Wizard>
  );
};

export default ConfigureLlm;
