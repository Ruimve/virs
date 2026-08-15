import { memo } from 'react';
import { Brand } from '@/components/Logo';
import { Theme } from '@/components/Theme';
import { Flame, ShieldCheck } from '@/components/Icon';
import { useBot } from '@/context/BotContext';
import { useLayout } from '@/context/LayoutContext';
import { usePaper } from '../../context/PaperContext';
import { formatCompact, formatInterval } from '../../Bot/Bot/components/utils';

interface StatusStyle {
  text: string;
  dot: string;
  bg: string;
  pulse: boolean;
}

const statusConfig = (status: string): StatusStyle => {
  const map: Record<string, StatusStyle> = {
    running: {
      text: '运行中',
      dot: 'bg-success',
      bg: 'bg-success-bg text-success-text',
      pulse: true,
    },
    paused: {
      text: '已暂停',
      dot: 'bg-warning',
      bg: 'bg-warning-bg text-warning-text',
      pulse: false,
    },
    stopped: {
      text: '已停止',
      dot: 'bg-on-surface-muted',
      bg: 'bg-surface-2 text-on-surface-tertiary',
      pulse: false,
    },
    draft: {
      text: '草稿',
      dot: 'bg-on-surface-faint',
      bg: 'bg-surface-2 text-on-surface-tertiary',
      pulse: false,
    },
    error: { text: '错误', dot: 'bg-danger', bg: 'bg-danger-bg text-danger-text', pulse: true },
  };
  return (
    map[status] || {
      text: status,
      dot: 'bg-on-surface-muted',
      bg: 'bg-surface-2 text-on-surface-tertiary',
      pulse: false,
    }
  );
};

export const TradeHeader = memo(() => {
  const { bot } = useBot();
  const { enabled: paperEnabled } = usePaper();
  const { actions, changeDrawerOpen } = useLayout();

  const sc = statusConfig(bot?.status || '');
  const interval = bot ? formatInterval(bot.decide_interval_secs) : '';

  return (
    <div className="relative z-10 flex items-center h-14 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
      {/* Mobile: Brand as drawer trigger */}
      <div className="md:hidden flex items-center pl-3 shrink-0">
        <Brand size={20} onClick={() => changeDrawerOpen(true)} />
      </div>

      {/* Left: bot name + badges (desktop) */}
      <div className="hidden md:flex items-center gap-3 pl-4 min-w-0 shrink-0">
        {bot && (
          <>
            <h1 className="text-sm font-display font-semibold text-on-surface truncate">
              {bot.name}
            </h1>

            <span
              className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-2xs font-medium tracking-wide shrink-0 ${sc.bg} ${sc.pulse ? 'animate-subtle-pulse' : ''}`}
            >
              <span className={`w-1.5 h-1.5 rounded-full ${sc.dot}`} />
              {sc.text}
            </span>

            {paperEnabled !== null && (
              <span
                className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-2xs font-medium tracking-wide shrink-0 ${
                  paperEnabled
                    ? 'bg-info-bg text-info-text border border-info-border'
                    : 'bg-warning-bg text-warning-text border border-warning-border'
                }`}
              >
                {paperEnabled ? (
                  <ShieldCheck className="w-3 h-3" strokeWidth={2} />
                ) : (
                  <Flame className="w-3 h-3" strokeWidth={2} />
                )}
                {paperEnabled ? 'Paper' : 'Live'}
              </span>
            )}

            {bot.ai_analysis && (
              <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-2xs font-medium tracking-wide shrink-0 bg-ai-bg text-ai border border-ai-border">
                <span className="inline-block w-1.5 h-1.5 rounded-full bg-ai animate-cursor-blink" />
                AI 在线
              </span>
            )}

            <div className="hidden lg:flex items-center gap-2 text-2xs text-on-surface-tertiary shrink-0">
              <span className="font-mono tabular-nums">{bot.leverage}x</span>
              <span className="text-on-surface-faint">·</span>
              {interval && <span>{interval}</span>}
              <span className="text-on-surface-faint">·</span>
              <span className="font-mono tabular-nums">{formatCompact(bot.initial_capital)}</span>
            </div>
          </>
        )}
      </div>

      {/* Mobile: status badge */}
      <div className="md:hidden flex items-center gap-2 pl-2 shrink-0">
        <span
          className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-2xs font-medium tracking-wide ${sc.bg} ${sc.pulse ? 'animate-subtle-pulse' : ''}`}
        >
          <span className={`w-1.5 h-1.5 rounded-full ${sc.dot}`} />
          {sc.text}
        </span>
      </div>

      {/* Right: actions + theme */}
      <div className="flex items-center gap-1.5 pr-3 md:pr-4 ml-auto shrink-0">
        {actions.map((action) => (
          <button
            key={action.key}
            type="button"
            onClick={action.onClick}
            className={`px-3 py-1.5 rounded-lg text-caption font-medium tracking-wide transition-colors duration-200 cursor-pointer ${action.className || ''}`}
          >
            {action.label}
          </button>
        ))}
        <Theme />
      </div>
    </div>
  );
});

export default TradeHeader;
