import { useCallback, useState } from 'react';
import Logo from '@/components/Logo';
import Theme from '@/components/Theme';
import { Close } from '@/components/Icon';
import { useHeader } from './HeaderContext';
import { usePaper } from '../../context/PaperContext';
import { useBot } from '../../context/BotContext';

interface StatusStyle {
  text: string;
  dot: string;
  bg: string;
}

const statusConfig = (status: string): StatusStyle => {
  const map: Record<string, StatusStyle> = {
    running: { text: '运行中', dot: 'bg-emerald-500', bg: 'bg-emerald-500/10 text-emerald-400' },
    paused: { text: '已暂停', dot: 'bg-amber-500', bg: 'bg-amber-500/10 text-amber-400' },
    stopped: {
      text: '已停止',
      dot: 'bg-on-surface-muted',
      bg: 'bg-surface-2 text-on-surface-tertiary',
    },
    draft: {
      text: '草稿',
      dot: 'bg-on-surface-faint',
      bg: 'bg-surface-2 text-on-surface-tertiary',
    },
    error: { text: '错误', dot: 'bg-red-500', bg: 'bg-red-500/10 text-red-400' },
  };
  return (
    map[status] || {
      text: status,
      dot: 'bg-on-surface-muted',
      bg: 'bg-surface-2 text-on-surface-tertiary',
    }
  );
};

const Header = () => {
  const { bot } = useBot();
  const { tabs, activeTab, actions } = useHeader();
  const { enabled: paperMode } = usePaper();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [drawerClosing, setDrawerClosing] = useState(false);

  const closeDrawer = () => {
    setDrawerClosing(true);
  };

  const openDrawer = useCallback(() => {
    setDrawerOpen(true);
  }, []);

  const handleDrawerAnimEnd = () => {
    if (drawerClosing) {
      setDrawerOpen(false);
      setDrawerClosing(false);
    }
  };

  const sc = statusConfig(bot?.status || '');

  return (
    <>
      {/* Header bar */}
      <div className="relative z-10 flex items-center h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
        {/* Left: logo (clickable on mobile to open drawer) + bot info */}
        <div className="flex items-center gap-2 pl-4 md:pl-8 shrink-0">
          <Logo onClick={openDrawer} className="md:hidden" />
          <Logo className="hidden md:block" />
          <div className="flex items-center gap-2">
            <span
              className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium ${sc.bg}`}
            >
              <span className={`w-1 h-1 rounded-full ${sc.dot} `} />
              {sc.text}
            </span>
            <span
              className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium ${paperMode ? 'bg-amber-500/10 text-amber-400' : 'bg-emerald-500/10 text-emerald-400'}`}
            >
              {paperMode ? 'Paper' : '实盘'}
            </span>
            <span className="text-xs text-on-surface-tertiary md:hidden">
              {bot?.symbol || ''} · {bot?.leverage || ''}x
            </span>
            <span className="text-xs text-on-surface-tertiary hidden md:block">
              {bot?.symbol || ''} · {bot?.exchange?.toUpperCase()} · {bot?.leverage || ''}x
            </span>
          </div>
        </div>

        {/* Center: tabs (desktop) */}
        <div className="hidden md:flex items-center justify-center flex-1 gap-1">
          {tabs.map((tab) => (
            <button
              key={tab.key}
              onClick={() => tab.onClick(tab.key)}
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
          {actions.map((action) => {
            return (
              <button
                onClick={() => action.onClick(action.key)}
                className={`px-3 py-1.5 rounded-lg text-xs font-medium ${action.className || ''}`}
              >
                {action.label}
              </button>
            );
          })}
          <Theme />
        </div>

        {/* Right: theme toggle (mobile) */}
        <div className="md:hidden flex items-center pr-4 ml-auto shrink-0">
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
            <div className="absolute inset-0 bg-black/50" />
          </div>
          <div
            onAnimationEnd={handleDrawerAnimEnd}
            className={`fixed inset-y-0 left-0 z-50 w-64 bg-base border-r border-line-default shadow-xl md:hidden flex flex-col ${drawerClosing ? 'animate-drawer-out' : 'animate-drawer-in'}`}
          >
            <div className="flex items-center justify-between px-4 h-14 border-b border-line-subtle">
              <div className="flex items-center gap-2">
                <Logo />
              </div>
              <button
                onClick={closeDrawer}
                className="p-1.5 rounded-lg hover:bg-surface-2 text-on-surface-tertiary"
              >
                <Close className="w-4 h-4" strokeWidth={2} />
              </button>
            </div>

            <div className="flex-1 py-2">
              {tabs.map((tab) => (
                <button
                  key={tab.key}
                  onClick={() => {
                    tab.onClick(tab.key);
                    closeDrawer();
                  }}
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
              {actions.map((action) => {
                return (
                  <button
                    onClick={() => {
                      action.onClick!(action.key);
                      closeDrawer();
                    }}
                    className={`w-full px-3 py-2 rounded-lg text-xs font-medium ${action.className || ''}`}
                  >
                    {action.label}
                  </button>
                );
              })}
            </div>
          </div>
        </>
      )}
    </>
  );
};

export default Header;
