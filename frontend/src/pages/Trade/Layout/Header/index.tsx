import { useCallback, useState } from 'react';
import Logo from '@/components/Logo';
import Theme from '@/components/Theme';
import { Close, Flame, ShieldCheck } from '@/components/Icon';
import IconBtn from '@/components/IconBtn';
import { useHeader } from './HeaderContext';
import { usePaper } from '../../context/PaperContext';
import { useBot } from '../../context/BotContext';

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
    error: {
      text: '错误',
      dot: 'bg-danger',
      bg: 'bg-danger-bg text-danger-text',
      pulse: true,
    },
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

const Header = () => {
  const { bot } = useBot();
  const { tabs, activeTab, actions } = useHeader();
  const { enabled: paperEnabled } = usePaper();

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [drawerClosing, setDrawerClosing] = useState(false);

  const closeDrawer = useCallback(() => {
    setDrawerClosing(true);
  }, []);

  const openDrawer = useCallback(() => {
    setDrawerOpen(true);
  }, []);

  const handleDrawerAnimEnd = useCallback(() => {
    if (drawerClosing) {
      setDrawerOpen(false);
      setDrawerClosing(false);
    }
  }, [drawerClosing]);

  const sc = statusConfig(bot?.status || '');

  return (
    <>
      {/* Header bar — slimmer (h-12) */}
      <div className="relative z-10 flex items-center h-12 md:h-12 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
        {/* Left: logo + status + mode */}
        <div className="flex items-center gap-2.5 pl-3 md:pl-6 shrink-0">
          <Logo onClick={openDrawer} className="md:hidden" />
          <Logo className="hidden md:block" />

          {/* Status badge */}
          <span
            className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-[10px] font-medium tracking-wide ${sc.bg} ${
              sc.pulse ? 'animate-subtle-pulse' : ''
            }`}
          >
            <span className={`w-1.5 h-1.5 rounded-full ${sc.dot}`} />
            {sc.text}
          </span>

          {/* Paper/Live mode indicator — distinct pill */}
          {paperEnabled !== null && (
            <span
              className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-[10px] font-medium tracking-wide ${
                paperEnabled
                  ? 'bg-info/[0.08] text-info border border-info/20'
                  : 'bg-warning/[0.08] text-warning-text border border-warning/20'
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

          {/* Symbol + leverage */}
          <span className="text-[11px] text-on-surface-tertiary font-mono tabular-nums">
            {bot?.symbol || ''} &middot; {bot?.leverage || ''}x
          </span>
        </div>

        {/* Center: tabs (desktop) — pill style with smooth indicator */}
        <div className="hidden md:flex items-center justify-center flex-1 gap-0.5">
          {tabs.map((tab) => (
            <div
              key={tab.key}
              onClick={() => tab.onClick(tab.key)}
              className={`px-4 py-1.5 rounded-lg text-[11px] font-medium tracking-wide transition-all duration-200 cursor-pointer ${
                activeTab === tab.key
                  ? 'bg-accent-light text-accent shadow-sm'
                  : 'text-on-surface-tertiary hover:text-on-surface-secondary hover:bg-surface-2/50'
              }`}
            >
              {tab.label}
            </div>
          ))}
        </div>

        {/* Right: actions (desktop) */}
        <div className="hidden md:flex items-center gap-1.5 pr-6 shrink-0">
          {actions.map((action) => {
            return (
              <div
                onClick={() => action.onClick(action.key)}
                className={`px-3 py-1.5 rounded-lg text-[11px] font-medium tracking-wide transition-colors duration-200 cursor-pointer ${action.className || ''}`}
              >
                {action.label}
              </div>
            );
          })}
          <Theme />
        </div>

        {/* Right: theme toggle (mobile) */}
        <div className="md:hidden flex items-center pr-3 ml-auto shrink-0">
          <Theme />
        </div>
      </div>

      {/* Mobile drawer overlay */}
      {drawerOpen && (
        <>
          <div
            className={`fixed inset-0 z-40 md:hidden ${drawerClosing ? 'animate-fade-out' : ''}`}
            onClick={closeDrawer}
          >
            <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />
          </div>
          <div
            onAnimationEnd={handleDrawerAnimEnd}
            className={`fixed inset-y-0 left-0 z-50 w-72 bg-base border-r border-line-subtle shadow-2xl md:hidden flex flex-col ${
              drawerClosing ? 'animate-drawer-out' : 'animate-drawer-in'
            }`}
          >
            {/* Drawer header */}
            <div className="flex items-center justify-between px-4 h-12 border-b border-line-subtle shrink-0">
              <div className="flex items-center gap-2">
                <Logo />
              </div>
              <IconBtn onClick={closeDrawer}>
                <Close className="w-4 h-4" strokeWidth={2} />
              </IconBtn>
            </div>

            {/* Navigation section */}
            <div className="flex-1 overflow-y-auto py-2">
              <div className="px-4 py-1.5">
                <p className="text-[10px] uppercase tracking-[0.15em] text-on-surface-faint font-medium">
                  Navigation
                </p>
              </div>
              {tabs.map((tab) => (
                <div
                  key={tab.key}
                  onClick={() => {
                    tab.onClick(tab.key);
                    closeDrawer();
                  }}
                  className={`w-full text-left px-5 py-2.5 text-[12px] font-medium tracking-wide transition-colors cursor-pointer ${
                    activeTab === tab.key
                      ? 'text-accent bg-accent-light border-r-2 border-accent'
                      : 'text-on-surface-secondary hover:bg-surface-2/50 hover:text-on-surface'
                  }`}
                >
                  {tab.label}
                </div>
              ))}
            </div>

            {/* Actions section */}
            <div className="border-t border-line-subtle py-3 px-4 shrink-0">
              <div className="mb-2">
                <p className="text-[10px] uppercase tracking-[0.15em] text-on-surface-faint font-medium">
                  Actions
                </p>
              </div>
              <div className="space-y-1">
                {actions.map((action) => {
                  return (
                    <div
                      onClick={() => {
                        action.onClick!(action.key);
                        closeDrawer();
                      }}
                      className={`w-full px-3 py-2 rounded-lg text-[11px] font-medium transition-colors cursor-pointer ${action.className || ''}`}
                    >
                      {action.label}
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </>
      )}
    </>
  );
};

export default Header;
