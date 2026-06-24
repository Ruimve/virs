import { useEffect, memo } from 'react'
import { Outlet, useNavigate } from 'react-router-dom'
import { startAutoBot, stopAutoBot, deleteAutoBot } from '@/service/bot'
import { useBot } from '../context/BotContext'
import { useHeader, type ItemConfig } from '../components/Header/context'

const AutoBot = () => {
  const navigate = useNavigate()
  const { updateTabs, updateActions } = useHeader()
  const { bot, loading } = useBot()

  useEffect(() => {
    if (!bot?.id) return

    updateTabs([
      {
        key: 'bot',
        label: '机器人',
        onClick: () => {
          navigate(`/trade/auto/${bot?.id}/bot`, { replace: true })
        },
      },
      {
        key: 'trades',
        label: '交易记录',
        onClick: () => {
          navigate(`/trade/auto/${bot?.id}/trades`, { replace: true })
        },
      },
      {
        key: 'log',
        label: 'AI 决策日志',
        onClick: () => {
          navigate(`/trade/auto/${bot?.id}/log`, { replace: true })
        },
      },
      {
        key: 'system',
        label: '系统',
        onClick: () => {
          navigate(`/trade/system`, { replace: true })
        },
      },
    ])
  }, [bot?.id])

  useEffect(() => {
    if (!bot?.id || !bot?.status) return

    const actions: ItemConfig[] = []
    if (bot?.status === 'running') {
      actions.push({
        key: 'stop',
        label: '暂停',
        className:
          'bg-surface-1 border-line-default text-on-surface-tertiary hover:text-red-400 hover:border-red-500/20 transition-colors',
        onClick: async () => {
          await stopAutoBot(bot?.id)
        },
      })
    }

    if (bot?.status === 'stopped') {
      actions.push({
        key: 'start',
        label: '运行',
        className:
          'bg-emerald-500/10 border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/20 transition-colors',
        onClick: async () => {
          await startAutoBot(bot?.id)
        },
      })
    }

    actions.push({
      key: 'delete',
      label: '删除',
      className:
        'bg-red-500/10 border-red-500/20 text-red-400 hover:bg-red-500/20 transition-colors',
      onClick: async () => {
        if (!confirm('确定删除此机器人？将平仓所有持仓。')) return
        await deleteAutoBot(bot?.id)
        navigate('/setup/bot-type', { replace: true })
      },
    })

    updateActions(actions)
  }, [bot?.id, bot?.status])

  if (loading) {
    return (
      <div className="h-screen bg-base flex flex-col items-center justify-center relative gap-4">
        <svg
          className="animate-spin h-6 w-6 text-on-surface-tertiary"
          viewBox="0 0 24 24"
          fill="none"
        >
          <circle
            className="opacity-25"
            cx="12"
            cy="12"
            r="10"
            stroke="currentColor"
            strokeWidth="4"
          />
          <path
            className="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
          />
        </svg>
      </div>
    )
  }

  if (!bot?.id) {
    return (
      <div className="h-screen bg-base flex flex-col items-center justify-center relative gap-4">
        <div className="text-on-surface-tertiary text-sm">{'机器人不存在或加载失败'}</div>
        <button
          onClick={() => navigate('/setup/bot-type', { replace: true })}
          className="px-4 py-2 text-xs font-medium border border-line-default rounded-lg text-on-surface-tertiary hover:bg-surface-2 transition-colors"
        >
          创建新机器人
        </button>
      </div>
    )
  }

  return <Outlet />
}

export default memo(AutoBot)
