import { useCallback, useMemo, useState, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import { ShieldCheck, Flame, Warning } from '@/components/Icon';
import { Button } from '@/components/Button';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import { createGridBot, createAutoBot, startGridBot, startAutoBot } from '../../../service';
import { Title } from '@/components/Title';
import { WizardStep } from '../context/WizardContext/consts';

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

    const isGrid = wizard.bot_type === 'grid';
    const botParams = wizard.bot_params;
    try {
      // Step 1: Create bot
      let botId: string;
      if (isGrid) {
        const result = await createGridBot({
          symbol: botParams.symbol,
          exchange: wizard.exchange,
          grid_count: parseInt(botParams.grid_levels || '10'),
          upper_price: parseFloat(botParams.upper_price || '0'),
          lower_price: parseFloat(botParams.lower_price || '0'),
          quantity_per_grid: parseFloat(botParams.investment || '0'),
          leverage: parseInt(botParams.leverage || '5'),
          name: `Grid ${botParams.symbol || 'Bot'}`,
          paper_mode: paperMode,
        });
        if (!result.success || !result.data?.id) {
          setLaunchError(`Failed to create grid bot: ${result.error || 'Unknown error'}`);
          return;
        }
        botId = result.data.id;
      } else {
        const result = await createAutoBot({
          symbol: botParams.symbol,
          exchange: wizard.exchange,
          leverage: parseInt(botParams.leverage || '10'),
          decide_interval_secs: parseInt(botParams.decision_interval || '300'),
          name: `Auto ${botParams.symbol || 'Bot'}`,
          paper_mode: paperMode,
        });
        if (!result.success || !result.data?.id) {
          setLaunchError(`Failed to create auto bot: ${result.error || 'Unknown error'}`);
          return;
        }
        botId = result.data.id;
      }

      // Step 2: Start bot
      if (isGrid) {
        const result = await startGridBot(botId);
        if (!result.success) {
          setLaunchError(`Bot created but failed to start: ${result.error || 'Unknown error'}`);
          return;
        }
      } else {
        const result = await startAutoBot(botId);
        if (!result.success) {
          setLaunchError(`Bot created but failed to start: ${result.error || 'Unknown error'}`);
          return;
        }
      }

      // Step 3: Navigate to health check
      updateWizard({ paper_mode: paperMode, bot_id: botId });
      startTransition(() => {
        navigate(`/trade/${wizard.bot_type}/${botId}/health`, { replace: true });
      });
    } catch (err) {
      setLaunchError(`Unexpected error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setLaunching(false);
    }
  }, [wizard.bot_type, wizard.bot_params, wizard.exchange, paperMode, navigate, updateWizard]);

  const handleBack = useCallback(() => {
    navigate('/setup/params', { replace: true });
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

  const tradeMode = useMemo(() => {
    return (
      <div className="rounded-xl border border-line-subtle bg-surface-1/50 p-5">
        <Title className="mb-4">Trading Mode</Title>
        <div className="flex flex-col sm:flex-row gap-3">
          {/* Paper mode card */}
          <div
            onClick={() => setPaperMode(true)}
            className={`flex-1 p-5 rounded-xl border text-left transition-all duration-300 cursor-pointer backdrop-blur-sm ${
              paperMode
                ? 'bg-info/[0.06] border-info/30 shadow-md shadow-info/5'
                : 'bg-surface-1/40 border-line-default hover:bg-surface-2/40 hover:border-line-strong'
            }`}
          >
            <div className="flex items-start gap-3.5">
              <div
                className={`shrink-0 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-300 ${
                  paperMode ? 'bg-info/[0.12] text-info' : 'bg-surface-2/50 text-on-surface-faint'
                }`}
              >
                <ShieldCheck className="w-5 h-5" strokeWidth={1.8} />
              </div>
              <div className="flex-1">
                <p
                  className={`text-sm font-medium transition-colors duration-200 ${
                    paperMode ? 'text-on-base' : 'text-on-surface-tertiary'
                  }`}
                >
                  Paper Trading
                </p>
                <p className="text-xs text-on-surface-muted mt-1 leading-relaxed">
                  Simulated orders, no real funds at risk
                </p>
              </div>
              {paperMode && (
                <div className="shrink-0 w-5 h-5 rounded-full bg-info flex items-center justify-center mt-0.5">
                  <svg
                    className="w-3 h-3 text-white"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth={3}
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                  </svg>
                </div>
              )}
            </div>
          </div>

          {/* Real mode card */}
          <div
            onClick={() => setPaperMode(false)}
            className={`flex-1 p-5 rounded-xl border text-left transition-all duration-300 cursor-pointer backdrop-blur-sm ${
              !paperMode
                ? 'bg-warning/[0.06] border-warning-border animate-border-pulse shadow-md shadow-warning/5'
                : 'bg-surface-1/40 border-line-default hover:bg-surface-2/40 hover:border-line-strong'
            }`}
          >
            <div className="flex items-start gap-3.5">
              <div
                className={`shrink-0 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-300 ${
                  !paperMode
                    ? 'bg-warning/[0.15] text-warning-text'
                    : 'bg-surface-2/50 text-on-surface-faint'
                }`}
              >
                <Flame className="w-5 h-5" strokeWidth={1.8} />
              </div>
              <div className="flex-1">
                <p
                  className={`text-sm font-medium transition-colors duration-200 ${
                    !paperMode ? 'text-on-base' : 'text-on-surface-tertiary'
                  }`}
                >
                  Real Trading
                </p>
                <p className="text-xs text-on-surface-muted mt-1 leading-relaxed">
                  Live orders with real funds
                </p>
              </div>
              {paperMode && (
                <div className="shrink-0 w-5 h-5 rounded-full bg-surface-2/50 flex items-center justify-center mt-0.5" />
              )}
            </div>
          </div>
        </div>

        {/* Real mode warning */}
        {!paperMode && (
          <div className="mt-3 flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-warning/[0.06] border border-warning-border/50">
            <Warning className="w-4 h-4 text-warning-text shrink-0 mt-0.5" strokeWidth={1.8} />
            <p className="text-xs text-warning-text leading-relaxed">
              Real trading involves genuine financial risk. Ensure your configuration is correct
              before launching.
            </p>
          </div>
        )}
      </div>
    );
  }, [paperMode]);

  const summary = useMemo(() => {
    const isGrid = wizard.bot_type === 'grid';
    const botParams = wizard.bot_params;
    const rows = [
      { label: 'Exchange', value: wizard.exchange },
      { label: 'Strategy', value: isGrid ? 'Grid Bot' : 'Auto Bot' },
      { label: 'Symbol', value: botParams.symbol || '-' },
      { label: 'AI Model', value: wizard.llm_model },
      ...(isGrid
        ? [
            { label: 'Grid Levels', value: botParams.grid_levels || '-' },
            {
              label: 'Price Range',
              value: `${botParams.lower_price || '-'} ~ ${botParams.upper_price || '-'}`,
            },
            { label: 'Investment', value: `${botParams.investment || '-'} USDT` },
          ]
        : []),
      { label: 'Leverage', value: `${botParams.leverage || '-'}x` },
      ...(!isGrid
        ? [{ label: 'Decision Interval', value: `${botParams.decision_interval || '300'}s` }]
        : []),
      {
        label: 'Mode',
        value: paperMode ? 'Paper' : 'Real',
        valueClass: paperMode ? 'text-info' : 'text-warning-text',
      },
    ];

    return (
      <div className="rounded-xl bg-surface-1/50 border border-line-subtle p-5">
        <Title className="mb-4">Summary</Title>
        <div className="space-y-0">
          {rows.map((row, i) => (
            <div
              className={`flex items-center justify-between px-4 py-3 ${i !== rows.length - 1 ? 'border-b border-line-subtle/50' : ''}`}
            >
              <span className="text-xs text-on-surface-tertiary">{row.label}</span>
              <span
                className={`text-xs font-mono tabular-nums ${row.valueClass || 'text-on-surface-secondary'}`}
              >
                {row.value}
              </span>
            </div>
          ))}
        </div>
      </div>
    );
  }, [wizard.bot_params, wizard.bot_type, wizard.exchange, wizard.llm_model, paperMode]);

  return (
    <Wizard
      step={WizardStep.ReviewLaunch}
      title="Review & Launch"
      subtitle="Confirm your configuration and launch the bot"
      actions={actions}
    >
      <div className="space-y-5">
        {/* Trading Mode */}
        {tradeMode}

        {/* Summary */}
        {summary}

        {/* Launch error */}
        {launchError && (
          <div className="animate-error-enter p-3.5 bg-danger-bg border border-danger-border rounded-xl text-sm text-danger-text">
            {launchError}
          </div>
        )}
      </div>
    </Wizard>
  );
};

export default ReviewLaunch;
