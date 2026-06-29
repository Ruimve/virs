import { useCallback, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Spinner, ShieldCheck, Flame } from '@/components/Icon';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard, useWizardGuard } from '../context/WizardContext';
import { createGridBot, createAutoBot, startGridBot, startAutoBot } from '../../../service';
import { WizardStep } from '../context/WizardContext/consts';

const ReviewLaunch = () => {
  const navigate = useNavigate();
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
          market_type: wizard.market_type,
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
          market_type: wizard.market_type,
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
      navigate(`/trade/${wizard.bot_type}/${botId}/health`, { replace: true });
    } catch (err) {
      setLaunchError(`Unexpected error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setLaunching(false);
    }
  }, [
    wizard.bot_type,
    wizard.bot_params,
    wizard.exchange,
    wizard.market_type,
    paperMode,
    navigate,
    updateWizard,
  ]);

  const actions = useMemo(() => {
    return (
      <>
        <button
          onClick={() => navigate('/setup/params', { replace: true })}
          disabled={launching}
          className="w-full sm:w-auto sm:px-5 py-2.5 text-sm text-on-surface-tertiary hover:text-on-surface-secondary rounded-xl transition-colors duration-200 disabled:opacity-30"
        >
          Back
        </button>
        <button
          onClick={handleLaunch}
          disabled={launching}
          className="w-full sm:w-auto sm:px-6 py-2.5 bg-accent/80 hover:bg-accent-hover text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
        >
          {launching ? (
            <span className="flex items-center justify-center gap-2">
              <Spinner className="w-4 h-4" />
              Launching...
            </span>
          ) : (
            'Launch Bot'
          )}
        </button>
      </>
    );
  }, [launching, handleLaunch, navigate]);

  const tradeMode = useMemo(() => {
    return (
      <div className="p-4 rounded-xl border border-line-default bg-surface-1">
        <p className="text-[11px] tracking-[0.15em] text-on-surface-muted uppercase mb-3">
          Trading Mode
        </p>
        <div className="flex flex-col sm:flex-row gap-3">
          <button
            onClick={() => setPaperMode(true)}
            className={`flex-1 p-4 rounded-xl border text-left transition-all duration-200 ${
              paperMode
                ? 'bg-accent-light border-accent-muted ring-1 ring-accent-muted'
                : 'bg-surface-1 border-line-default hover:bg-surface-2'
            }`}
          >
            <div className="flex items-center gap-3">
              <div
                className={`w-8 h-8 rounded-lg flex items-center justify-center ${
                  paperMode ? 'bg-accent-muted text-accent' : 'bg-surface-2 text-on-surface-faint'
                }`}
              >
                <ShieldCheck className="w-4 h-4" strokeWidth={2} />
              </div>
              <div>
                <p
                  className={`text-sm font-medium ${paperMode ? 'text-on-base' : 'text-on-surface-tertiary'}`}
                >
                  Paper Trading
                </p>
                <p className="text-xs text-on-surface-muted mt-0.5">
                  Simulated orders, no real funds at risk
                </p>
              </div>
            </div>
          </button>
          <button
            onClick={() => setPaperMode(false)}
            className={`flex-1 p-4 rounded-xl border text-left transition-all duration-200 ${
              !paperMode
                ? 'bg-warning-bg border-warning-border ring-1 ring-warning/20'
                : 'bg-surface-1 border-line-default hover:bg-surface-2'
            }`}
          >
            <div className="flex items-center gap-3">
              <div
                className={`w-8 h-8 rounded-lg flex items-center justify-center ${
                  !paperMode
                    ? 'bg-warning/20 text-warning-text'
                    : 'bg-surface-2 text-on-surface-faint'
                }`}
              >
                <Flame className="w-4 h-4" strokeWidth={2} />
              </div>
              <div>
                <p
                  className={`text-sm font-medium ${!paperMode ? 'text-on-base' : 'text-on-surface-tertiary'}`}
                >
                  Real Trading
                </p>
                <p className="text-xs text-on-surface-muted mt-0.5">Live orders with real funds</p>
              </div>
            </div>
          </button>
        </div>
      </div>
    );
  }, [paperMode]);

  const summary = useMemo(() => {
    const isGrid = wizard.bot_type === 'grid';
    const botParams = wizard.bot_params;
    return (
      <div className="p-4 rounded-xl bg-surface-1 border border-line-default">
        <p className="text-[11px] tracking-[0.15em] text-on-surface-muted uppercase mb-3">
          Summary
        </p>
        <div className="space-y-1.5">
          <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
            <span className="text-[12px] text-on-surface-tertiary">Exchange</span>
            <span className="text-[12px] text-on-surface-secondary font-mono">
              {wizard.exchange}
            </span>
          </div>
          <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
            <span className="text-[12px] text-on-surface-tertiary">Strategy</span>
            <span className="text-[12px] text-on-surface-secondary font-mono">
              {isGrid ? 'Grid Bot' : 'Auto Bot'}
            </span>
          </div>
          <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
            <span className="text-[12px] text-on-surface-tertiary">Market</span>
            <span className="text-[12px] text-on-surface-secondary font-mono">
              {wizard.market_type}
            </span>
          </div>
          <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
            <span className="text-[12px] text-on-surface-tertiary">Symbol</span>
            <span className="text-[12px] text-on-surface-secondary font-mono">
              {botParams.symbol || '-'}
            </span>
          </div>
          <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
            <span className="text-[12px] text-on-surface-tertiary">AI Model</span>
            <span className="text-[12px] text-on-surface-secondary font-mono">
              {wizard.llm_model}
            </span>
          </div>
          {isGrid && (
            <>
              <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
                <span className="text-[12px] text-on-surface-tertiary">Grid Levels</span>
                <span className="text-[12px] text-on-surface-secondary font-mono">
                  {botParams.grid_levels || '-'}
                </span>
              </div>
              <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
                <span className="text-[12px] text-on-surface-tertiary">Price Range</span>
                <span className="text-[12px] text-on-surface-secondary font-mono">
                  {botParams.lower_price || '-'} ~ {botParams.upper_price || '-'}
                </span>
              </div>
              <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
                <span className="text-[12px] text-on-surface-tertiary">Investment</span>
                <span className="text-[12px] text-on-surface-secondary font-mono">
                  {botParams.investment || '-'} USDT
                </span>
              </div>
            </>
          )}
          <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
            <span className="text-[12px] text-on-surface-tertiary">Leverage</span>
            <span className="text-[12px] text-on-surface-secondary font-mono">
              {botParams.leverage || '-'}x
            </span>
          </div>
          {!isGrid && (
            <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
              <span className="text-[12px] text-on-surface-tertiary">Decision Interval</span>
              <span className="text-[12px] text-on-surface-secondary font-mono">
                {botParams.decision_interval || '300'}s
              </span>
            </div>
          )}
          <div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
            <span className="text-[12px] text-on-surface-tertiary">Mode</span>
            <span
              className={`text-[12px] font-mono font-medium ${paperMode ? 'text-accent' : 'text-warning-text'}`}
            >
              {paperMode ? 'Paper' : 'Real'}
            </span>
          </div>
        </div>
      </div>
    );
  }, [
    wizard.bot_params,
    wizard.bot_type,
    wizard.exchange,
    wizard.llm_model,
    wizard.market_type,
    paperMode,
  ]);

  return (
    <Wizard
      step={WizardStep.ReviewLaunch}
      title="Review & Launch"
      subtitle="Confirm your configuration and launch the bot"
      actions={actions}
    >
      <div className="space-y-6">
        {/* Trading Mode */}
        {tradeMode}

        {/* Summary */}
        {summary}

        {/* Launch error */}
        {launchError && (
          <div className="p-3 bg-danger-bg border border-danger-border rounded-xl text-sm text-danger-text">
            {launchError}
          </div>
        )}
      </div>
    </Wizard>
  );
};

export default ReviewLaunch;
