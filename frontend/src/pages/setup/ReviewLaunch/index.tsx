import { useCallback, useMemo, useState, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import { Alert } from '@/components/Alert';
import { Button } from '@/components/Button';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import { createAutoBot, startAutoBot } from '../../../service';
import { WizardStep } from '../context/WizardContext/define';
import { FormCard, FormField, ToggleSwitch, ReviewRow } from '../components';

const AiBadge = () => (
  <span className="inline-flex items-center px-2 py-0.5 rounded-md text-2xs font-medium border bg-ai-bg text-ai border-ai-border ml-1">
    AI
  </span>
);

const ReviewLaunch = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { wizard, updateWizard } = useWizard();
  useWizardGuard(wizard.current_step, WizardStep.ReviewLaunch);

  const [paperMode, setPaperMode] = useState(wizard.paper_mode);
  const [launching, setLaunching] = useState(false);
  const [launchError, setLaunchError] = useState('');

  const handleLaunch = useCallback(async () => {
    setLaunching(true);
    setLaunchError('');

    try {
      const result = await createAutoBot({
        symbol: wizard.bot_params.symbol,
        exchange: wizard.exchange,
        leverage: Number(wizard.bot_params.leverage),
        max_position_pct: Number(wizard.bot_params.max_position_pct),
        decide_interval_secs: Number(wizard.bot_params.decision_interval),
        name: `Auto ${wizard.bot_params.symbol || 'Bot'}`,
        paper_mode: paperMode,
        auto_optimize: wizard.auto_optimize,
      });
      if (!result.success || !result.data?.id) {
        setLaunchError(`Failed to create auto bot: ${result.message || 'Unknown error'}`);
        return;
      }
      const botId = result.data.id;

      const startResult = await startAutoBot(botId);
      if (!startResult.success) {
        setLaunchError(
          `Bot created but failed to start: ${startResult.message || 'Unknown error'}`,
        );
        return;
      }

      updateWizard({ paper_mode: paperMode, bot_id: botId });
      startTransition(() => {
        navigate(`/trade/auto/${botId}/bot`, { replace: true });
      });
    } catch (err) {
      setLaunchError(`Unexpected error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setLaunching(false);
    }
  }, [wizard.bot_params, wizard.exchange, wizard.auto_optimize, paperMode, navigate, updateWizard]);

  const handleBack = useCallback(() => {
    navigate('/setup/optimization', { replace: true });
  }, [navigate]);

  const actions = useMemo(() => {
    return (
      <>
        <Button variant="ghost" onClick={handleBack} disabled={launching}>
          Back
        </Button>
        <Button variant="primary" onClick={handleLaunch} loading={launching || isPending}>
          {launching ? 'Launching...' : 'Launch Bot'}
        </Button>
      </>
    );
  }, [launching, isPending, handleBack, handleLaunch]);

  const riskLabel = useMemo(() => {
    const r = wizard.risk_tolerance;
    if (r === 'low') return 'Low';
    if (r === 'high') return 'High';
    return 'Medium';
  }, [wizard.risk_tolerance]);

  return (
    <Wizard
      step={WizardStep.ReviewLaunch}
      title="Review & Launch"
      subtitle="Confirm your configuration and launch the bot"
      actions={actions}
    >
      <FormCard>
        <FormField label="" noBorder>
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="text-sm font-medium text-on-base">Trading Mode</div>
              <div
                className={`text-xs mt-0.5 leading-relaxed ${paperMode ? 'text-on-surface-tertiary' : 'text-warning-text'}`}
              >
                {paperMode
                  ? 'Simulated orders, no real funds at risk'
                  : 'Live orders with real funds — proceed with caution'}
              </div>
            </div>
            <ToggleSwitch on={!paperMode} warning onClick={() => setPaperMode(!paperMode)} />
          </div>
        </FormField>

        <ReviewRow
          label="Bot Type"
          value={
            <>
              Auto Bot <AiBadge />
            </>
          }
        />
        <ReviewRow label="AI Model" value={wizard.llm_model} mono />
        <ReviewRow label="Exchange" value={wizard.exchange} />
        <ReviewRow
          label="Symbol · Leverage"
          value={`${wizard.bot_params.symbol} · ${wizard.bot_params.leverage}x`}
          mono
        />
        <ReviewRow label="Max Position" value={`${wizard.bot_params.max_position_pct}%`} mono />
        <ReviewRow
          label="Optimization"
          value={wizard.auto_optimize ? `Enabled · ${wizard.optimization_interval}` : 'Disabled'}
        />
        {wizard.auto_optimize && <ReviewRow label="Risk Tolerance" value={riskLabel} />}
      </FormCard>

      <div className="mt-5">
        <Alert type="success" title="All checks passed" className="mb-3" />

        {!paperMode && (
          <Alert
            type="warning"
            title="Real trading involves genuine financial risk. Ensure your configuration is correct
              before launching."
            className="mb-3"
          />
        )}

        {launchError && (
          <Alert type="danger" title="launchError" className="animate-error-enter mb-3" />
        )}
      </div>
    </Wizard>
  );
};

export default ReviewLaunch;
