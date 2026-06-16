import { useState } from 'react';
import { useTheme } from '../../../../lib/theme';
import VirsLogo from '../../../../components/VirsLogo';
import type { BotInfo, TabConfig, StatusStyle } from '../shared';
import { statusConfig } from '../shared';

interface BotDetailHeaderProps {
  bot: BotInfo;
  tabs: TabConfig[];
  activeTab: string;
  onTabChange: (key: string) => void;
  actions?: {
    onStop?: () => void;
    onStart?: () => void;
    onDelete?: () => void;
  };
  /** Whether to show start button when status is 'stopped' (grid: false, auto: true) */
  showStartWhenStopped?: boolean;
  /** Whether to show pulse animation on running status dot */
  pulseOnRunning?: boolean;
}

export default function BotDetailHeader({
  bot,
  tabs,
  activeTab,
  onTabChange,
  actions,
  showStartWhenStopped = true,
  pulseOnRunning = false,
}: BotDetailHeaderProps) {
  const { isDark, toggleTheme } = useTheme();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const sc: StatusStyle = statusConfig(bot.status);

  const canStart = showStartWhenStopped
    ? bot.status !== 'running'
    : bot.status !== 'running' && bot.status !== 'stopped';

  return (
    <>
      {/* Header bar */}
      <div className="relative z-10 flex items-center h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
        {/* Left: logo (clickable on mobile to open drawer) + bot info */}
        <div className="flex items-center gap-2 pl-4 md:pl-8 shrink-0">
          <button onClick={() => setDrawerOpen(true)} className="md:hidden">
            <VirsLogo />
          </button>
          <VirsLogo className="hidden md:block" />
          <div className="flex items-center gap-2">
            <span className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium ${sc.bg}`}>
              <span className={`w-1 h-1 rounded-full ${sc.dot} ${bot.status === 'running' && pulseOnRunning ? 'animate-pulse' : ''}`} />
              {sc.text}
            </span>
            <span className="text-xs text-on-surface-tertiary hidden sm:inline">
              {bot.symbol} · {bot.exchange.toUpperCase()} · {bot.leverage}x
            </span>
          </div>
        </div>

        {/* Center: tabs (desktop) */}
        <div className="hidden md:flex items-center justify-center flex-1 gap-1">
          {tabs.map((tab) => (
            <button
              key={tab.key}
              onClick={() => onTabChange(tab.key)}
              className={`px-4 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                activeTab === tab.key
                  ? 'bg-accent-light text-accent'
                  : 'text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2'
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {/* Right: actions (desktop) */}
        <div className="hidden md:flex items-center gap-2 pr-8 shrink-0">
          {actions && bot.status === 'running' && actions.onStop && (
            <button
              onClick={actions.onStop}
              className="px-3 py-1.5 rounded-lg text-xs font-medium bg-red-500/10 border border-red-500/20 text-red-400 hover:bg-red-500/20 transition-colors"
            >
              停止
            </button>
          )}
          {actions && canStart && actions.onStart && (
            <button
              onClick={actions.onStart}
              className="px-3 py-1.5 rounded-lg text-xs font-medium bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/20 transition-colors"
            >
              启动
            </button>
          )}
          {actions?.onDelete && (
            <button
              onClick={actions.onDelete}
              className="px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-1 border border-line-default text-on-surface-tertiary hover:text-red-400 hover:border-red-500/20 transition-colors"
            >
              删除
            </button>
          )}
          <ThemeToggle isDark={isDark} toggleTheme={toggleTheme} />
        </div>

        {/* Right: theme toggle (mobile) */}
        <div className="md:hidden flex items-center pr-4 ml-auto shrink-0">
          <ThemeToggle isDark={isDark} toggleTheme={toggleTheme} />
        </div>
      </div>

      {/* Mobile drawer overlay */}
      {drawerOpen && (
        <>
          <div className="fixed inset-0 z-40 md:hidden" onClick={() => setDrawerOpen(false)}>
            <div className="absolute inset-0 bg-black/50" />
          </div>
          <div className="fixed inset-y-0 left-0 z-50 w-64 bg-base border-r border-line-default shadow-xl md:hidden animate-fade-in">
            <div className="flex items-center justify-between px-4 h-14 border-b border-line-subtle">
              <div className="flex items-center gap-2">
                <VirsLogo />
              </div>
              <button onClick={() => setDrawerOpen(false)} className="p-1.5 rounded-lg hover:bg-surface-2 text-on-surface-tertiary">
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2">
                  <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <div className="py-2">
              {tabs.map((tab) => (
                <button
                  key={tab.key}
                  onClick={() => { onTabChange(tab.key); setDrawerOpen(false); }}
                  className={`w-full text-left px-4 py-3 text-sm transition-colors ${
                    activeTab === tab.key
                      ? 'text-accent bg-accent-light'
                      : 'text-on-surface-secondary hover:bg-surface-2'
                  }`}
                >
                  {tab.label}
                </button>
              ))}
            </div>

            <div className="border-t border-line-subtle py-3 px-4 space-y-2">
              {actions && bot.status === 'running' && actions.onStop && (
                <button onClick={() => { actions.onStop!(); setDrawerOpen(false); }} className="w-full px-3 py-2 rounded-lg text-xs font-medium bg-red-500/10 border border-red-500/20 text-red-400 hover:bg-red-500/20 transition-colors">
                  停止机器人
                </button>
              )}
              {actions && canStart && actions.onStart && (
                <button onClick={() => { actions.onStart!(); setDrawerOpen(false); }} className="w-full px-3 py-2 rounded-lg text-xs font-medium bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/20 transition-colors">
                  启动机器人
                </button>
              )}
              {actions?.onDelete && (
                <button onClick={() => { actions.onDelete!(); setDrawerOpen(false); }} className="w-full px-3 py-2 rounded-lg text-xs font-medium bg-surface-1 border border-line-default text-on-surface-tertiary hover:text-red-400 hover:border-red-500/20 transition-colors">
                  删除机器人
                </button>
              )}
            </div>
          </div>
        </>
      )}
    </>
  );
}

// ── Theme toggle icon ─────────────────────────────────────

function ThemeToggle({ isDark, toggleTheme }: { isDark: boolean; toggleTheme: () => void }) {
  return (
    <button
      onClick={toggleTheme}
      className="p-2 rounded-lg text-on-surface-tertiary hover:text-on-surface-secondary hover:bg-surface-2 transition-colors"
    >
      {isDark ? (
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2">
          <path strokeLinecap="round" strokeLinejoin="round" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
        </svg>
      ) : (
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2">
          <path strokeLinecap="round" strokeLinejoin="round" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
        </svg>
      )}
    </button>
  );
}
