import { useEffect } from 'react'
import { useNavigate, Outlet } from 'react-router-dom'
import { startGridBot, stopGridBot, deleteGridBot } from '@/service/bot'
import type { GridBot } from '@/service/types'
import { useBot } from '../context/BotContext'
import { useHeader, type ItemConfig } from '../components/Header/context'

export default function GridBot() {
  const navigate = useNavigate()
  const { updateTabs, updateActions } = useHeader()
  const { bot, loading } = useBot()

  useEffect(() => {
    if (!bot?.id) return

    updateTabs([
      {
        key: 'market',
        label: '行情',
        onClick: () => {
          navigate(`/trade/grid/${bot?.id}/bot`, { replace: true })
        },
      },
      {
        key: 'trades',
        label: '交易记录',
        onClick: () => {
          navigate(`/trade/grid/${bot?.id}/trades`, { replace: true })
        },
      },
      {
        key: 'log',
        label: 'AI 决策日志',
        onClick: () => {
          navigate(`/trade/grid/${bot?.id}/log`, { replace: true })
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
    actions.push({
      key: 'delete',
      label: '删除',
      onClick: async () => {
        if (!confirm('确定删除此机器人？')) return
        await stopGridBot(bot?.id).catch(() => {})
        await deleteGridBot(bot?.id)
        navigate('/setup/bot-type', { replace: true })
      },
    })
    if (bot?.status === 'running') {
      actions.push({
        key: 'stop',
        label: '暂停',
        onClick: async () => {
          await stopGridBot(bot?.id)
        },
      })
    }
    if (bot?.status === 'stopped') {
      actions.push({
        key: 'start',
        label: '运行',
        onClick: async () => {
          await startGridBot(bot?.id)
        },
      })
    }
    updateActions(actions)
  }, [bot?.id, bot?.status])

  if (loading || !bot) {
    return (
      <div className="flex-1 flex items-center justify-center relative z-10 h-screen bg-base">
        <svg
          className="animate-spin h-5 w-5 text-on-surface-tertiary"
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

  return <Outlet />
}
