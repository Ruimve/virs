import { useState, useEffect, useMemo, useCallback, useTransition } from 'react';
import { useNavigate } from 'react-router-dom';
import { Check, InfoCircle } from '@/components/Icon';
import { Wizard } from '../context/WizardContext/Wizard';
import { useWizard } from '../context/WizardContext';
import { findActiveBot } from '../../../service';
import { WizardStep } from '../context/WizardContext/consts';
import { Button } from '@/components/Button';

const BOT_TYPES = [
  {
    id: 'grid' as const,
    name: 'Grid Bot',
    desc: 'Profit from market volatility by placing orders at regular price intervals. Best for ranging markets.',
    features: ['Grid levels', 'Auto buy/sell', 'Range-bound profit'],
    iconColor: 'text-info',
    selectedBg: 'from-info/[0.06] to-accent/[0.04]',
    selectedBorder: 'border-info/30',
    glowColor: 'shadow-info/5',
  },
  {
    id: 'auto' as const,
    name: 'Auto Bot',
    desc: 'AI-driven fully automated trading. Analyzes market conditions and executes trades autonomously.',
    features: ['AI analysis', 'Auto execution', 'Multi-market'],
    iconColor: 'text-accent',
    selectedBg: 'from-accent/[0.06] to-accent-muted/[0.04]',
    selectedBorder: 'border-accent-muted/40',
    glowColor: 'shadow-accent/5',
  },
];

const GridIcon = ({ className }: { className?: string }) => (
  <svg
    className={className}
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    strokeWidth="1.2"
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      d="M3.75 6A2.25 2.25 0 016 3.75h2.25A2.25 2.25 0 0110.5 6v2.25a2.25 2.25 0 01-2.25 2.25H6a2.25 2.25 0 01-2.25-2.25V6zM3.75 15.75A2.25 2.25 0 016 13.5h2.25a2.25 2.25 0 012.25 2.25V18a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 18v-2.25zM13.5 6a2.25 2.25 0 012.25-2.25H18A2.25 2.25 0 0120.25 6v2.25A2.25 2.25 0 0118 10.5h-2.25a2.25 2.25 0 01-2.25-2.25V6zM13.5 15.75a2.25 2.25 0 012.25-2.25H18a2.25 2.25 0 012.25 2.25V18A2.25 2.25 0 0118 20.25h-2.25a2.25 2.25 0 01-2.25-18v-2.25z"
    />
    <line
      strokeLinecap="round"
      x1="12"
      y1="3.75"
      x2="12"
      y2="20.25"
      strokeWidth="0.8"
      opacity="0.4"
    />
    <line
      strokeLinecap="round"
      x1="3.75"
      y1="12"
      x2="20.25"
      y2="12"
      strokeWidth="0.8"
      opacity="0.4"
    />
  </svg>
);

const BrainIcon = ({ className }: { className?: string }) => (
  <svg
    className={className}
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    strokeWidth="1.2"
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      d="M15.59 14.37a6 6 0 01-5.84 7.38v-4.8m5.84-2.58a14.98 14.98 0 006.16-12.12A14.98 14.98 0 009.631 8.41m5.96 5.96a14.926 14.926 0 01-5.841 2.58m-.119-8.54a6 6 0 00-7.381 5.84h4.8m2.581-5.84a14.927 14.927 0 00-2.58 5.84m2.699 2.7c-.103.021-.207.041-.311.06a15.09 15.09 0 01-2.448-2.448 14.9 14.9 0 01.06-.312m-2.24 2.39a4.493 4.493 0 00-1.757 4.306 4.493 4.493 0 004.306-1.758M16.5 9a1.5 1.5 0 11-3 0 1.5 1.5 0 013 0z"
    />
  </svg>
);

const SelectBotType = () => {
  const navigate = useNavigate();
  const [isPending, startTransition] = useTransition();
  const { wizard, updateWizard, advanceStep } = useWizard();
  const [botType, setBotType] = useState<'grid' | 'auto'>(wizard.bot_type);
  const [existingBot, setExistingBot] = useState<{ id: string; bot_type: string } | null>(null);

  useEffect(() => {
    updateWizard({ current_step: WizardStep.SelectBotType });
    findActiveBot().then((bot) => {
      if (bot) setExistingBot(bot);
    });
  }, [updateWizard]);

  const renderExistedBot = useCallback(() => {
    return (
      <div className="p-5 rounded-xl border border-warning-border bg-warning/5 mb-8">
        <div className="flex items-start gap-3">
          <InfoCircle className="w-5 h-5 text-warning-text shrink-0 mt-0.5" strokeWidth={2} />
          <div>
            <p className="text-sm font-medium text-on-base mb-1">已有机器人</p>
            <p className="text-xs text-on-surface-tertiary mb-3">
              每个账号只能创建一个机器人，请先删除已有机器人。
            </p>
            <Button
              variant="accent-outline"
              size="small"
              responsive={false}
              onClick={() => {
                if (existingBot?.bot_type === 'auto') {
                  navigate(`/trade/auto/${existingBot?.id}/bot`, { replace: true });
                } else if (existingBot?.bot_type === 'grid') {
                  navigate(`/trade/grid/${existingBot?.id}/bot`, { replace: true });
                }
              }}
            >
              查看已有机器人
            </Button>
          </div>
        </div>
      </div>
    );
  }, [existingBot, navigate]);

  const renderNoBot = useCallback(() => {
    return (
      <div className="space-y-4 mb-8">
        {BOT_TYPES.map((bot) => {
          const isSelected = botType === bot.id;
          return (
            <div
              key={bot.id}
              onClick={() => setBotType(bot.id)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => e.key === 'Enter' && setBotType(bot.id)}
              className={`group w-full p-5 md:p-6 rounded-xl border text-left transition-all duration-300 backdrop-blur-sm ${
                isSelected
                  ? `bg-gradient-to-br ${bot.selectedBg} ${bot.selectedBorder} shadow-lg ${bot.glowColor}`
                  : 'bg-surface-1/60 border-line-default hover:bg-surface-2/60 hover:border-line-strong'
              }`}
            >
              <div className="flex items-start gap-4">
                <div
                  className={`shrink-0 w-12 h-12 rounded-xl flex items-center justify-center transition-all duration-300 ${
                    isSelected
                      ? `${bot.iconColor} bg-surface-2/80`
                      : 'text-on-surface-faint bg-surface-2/50'
                  }`}
                >
                  {bot.id === 'grid' ? (
                    <GridIcon className="w-6 h-6" />
                  ) : (
                    <BrainIcon className="w-6 h-6" />
                  )}
                </div>
                <div className="flex-1 min-w-0">
                  <p
                    className={`text-base font-medium transition-colors duration-200 ${
                      isSelected ? 'text-on-base' : 'text-on-surface-secondary'
                    }`}
                  >
                    {bot.name}
                  </p>
                  <p className="text-[13px] text-on-surface-tertiary mt-1.5 leading-relaxed">
                    {bot.desc}
                  </p>
                  <div className="flex flex-wrap gap-2 mt-3">
                    {bot.features.map((f) => (
                      <span
                        key={f}
                        className={`px-2.5 py-1 rounded-full text-[11px] font-medium transition-all duration-200 ${
                          isSelected
                            ? 'bg-surface-3/60 text-on-surface-secondary'
                            : 'bg-surface-2/50 text-on-surface-faint'
                        }`}
                      >
                        {f}
                      </span>
                    ))}
                  </div>
                </div>
                {isSelected && (
                  <div className="shrink-0 w-6 h-6 rounded-full bg-accent flex items-center justify-center shadow-sm shadow-accent/20 transition-all duration-300">
                    <Check className="w-3.5 h-3.5 text-white" strokeWidth={3} />
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    );
  }, [botType]);

  const handleContinue = useCallback(() => {
    updateWizard({ bot_type: botType });
    advanceStep(WizardStep.ConfigureLlm);
    startTransition(() => {
      navigate('/setup/llm', { replace: true });
    });
  }, [botType, updateWizard, advanceStep, navigate]);

  const actions = useMemo(() => {
    return (
      <Button variant="primary" onClick={handleContinue} loading={isPending}>
        Continue
      </Button>
    );
  }, [isPending, handleContinue]);

  return (
    <Wizard
      step={WizardStep.SelectBotType}
      title="Choose Strategy"
      subtitle="Select the trading strategy that fits your goals"
      actions={actions}
    >
      {existingBot ? renderExistedBot() : renderNoBot()}
    </Wizard>
  );
};

export default SelectBotType;
