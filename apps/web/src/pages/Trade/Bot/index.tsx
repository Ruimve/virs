import { useEffect, type ReactNode } from 'react';
import { Outlet, useNavigate } from 'react-router-dom';
import { startBot, stopBot, deleteBot } from '@/service';
import { useBot } from '@/context/BotContext';
import { useLayout, type ActionItem, type NavItem } from '@/context/LayoutContext';

const navIcons: Record<string, ReactNode> = {
  bot: (
    <svg
      className="w-5 h-5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth="1.8"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
      />
    </svg>
  ),
  trades: (
    <svg
      className="w-5 h-5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth="1.8"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"
      />
    </svg>
  ),
  log: (
    <svg
      className="w-5 h-5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth="1.8"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z"
      />
    </svg>
  ),
  system: (
    <svg
      className="w-5 h-5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth="1.8"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01"
      />
    </svg>
  ),
};

const Bot = () => {
  const navigate = useNavigate();
  const { setNavItems, setActions } = useLayout();
  const { bot } = useBot();

  useEffect(() => {
    const items: NavItem[] = [
      {
        key: 'bot',
        label: '机器人',
        icon: navIcons.bot,
        onClick: () => navigate(`/trade/bot/${bot.id}/bot`, { replace: true }),
      },
      {
        key: 'trades',
        label: '交易记录',
        icon: navIcons.trades,
        onClick: () => navigate(`/trade/bot/${bot.id}/trades`, { replace: true }),
      },
      {
        key: 'log',
        label: 'AI 决策日志',
        icon: navIcons.log,
        onClick: () => navigate(`/trade/bot/${bot.id}/log`, { replace: true }),
      },
      {
        key: 'system',
        label: '系统',
        icon: navIcons.system,
        onClick: () => navigate(`/trade/bot/${bot.id}/system`, { replace: true }),
      },
    ];
    setNavItems(items);
  }, [bot.id, navigate, setNavItems]);

  useEffect(() => {
    const actions: ActionItem[] = [];

    if (bot.status === 'running') {
      actions.push({
        key: 'stop',
        label: '暂停',
        className:
          'bg-surface-1 border-line-default text-on-surface-tertiary hover:text-danger-text hover:border-danger-border transition-colors',
        onClick: async () => {
          await stopBot({ id: bot.id });
        },
      });
    }

    if (bot.status === 'stopped') {
      actions.push({
        key: 'start',
        label: '运行',
        className:
          'bg-success-bg border-success-border text-success-text hover:bg-success/20 transition-colors',
        onClick: async () => {
          await startBot({ id: bot.id });
        },
      });
    }

    actions.push({
      key: 'delete',
      label: '删除',
      className:
        'bg-danger-bg border-danger-border text-danger-text hover:bg-danger/20 transition-colors',
      onClick: async () => {
        if (!confirm('确定删除此机器人？将平仓所有持仓。')) return;
        await deleteBot({ id: bot.id });
        navigate('/setup/bot-type', { replace: true });
      },
    });

    setActions(actions);
  }, [bot.id, bot.status, navigate, setActions]);

  return <Outlet />;
};

export default Bot;
