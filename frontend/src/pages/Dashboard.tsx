import { type Component, createSignal, createEffect, Show, For } from 'solid-js'
import { api } from '../lib/api'
import { useWs, type WsEvent, type NotificationEvent } from '../lib/ws'

interface DashboardSummary {
  bots: {
    total: number
    running: number
    stopped: number
  }
  trades: {
    total: number
    total_pnl: number
  }
  exchanges: string[]
}

interface Trade {
  id: string
  bot_id: string
  symbol: string
  exchange: string
  side: string
  grid_level: number
  price: number
  quantity: number
  pnl: number
  pnl_pct: number
  status: string
  created_at: string
}

const Dashboard: Component = () => {
  const [summary, setSummary] = createSignal<DashboardSummary | null>(null)
  const [trades, setTrades] = createSignal<Trade[]>([])
  const [loading, setLoading] = createSignal(true)
  const [error, setError] = createSignal('')

  const [notifications, setNotifications] = createSignal<Array<{ id: number; level: string; message: string }>>([])
  let notifId = 0

  useWs((event: WsEvent) => {
    if (event.type === 'notification') {
      const notif = event as NotificationEvent
      setNotifications(prev => [
        { id: ++notifId, level: notif.data.level, message: notif.data.message },
        ...prev.slice(0, 9),
      ])
    }
    setTimeout(() => {
      setNotifications(prev => prev.slice(1))
    }, 5000)
  })

  createEffect(() => {
    loadDashboard()
  })

  async function loadDashboard() {
    setLoading(true)
    setError('')

    try {
      const [summaryResult, tradesResult] = await Promise.all([
        api.get<DashboardSummary>('/dashboard/summary'),
        api.get<{ items: Trade[] }>('/trades'),
      ])

      if (summaryResult.success && summaryResult.data) {
        setSummary(summaryResult.data)
      }

      if (tradesResult.success && tradesResult.data) {
        setTrades(tradesResult.data.items || [])
      }
    } catch {
      setError('加载数据失败')
    } finally {
      setLoading(false)
    }
  }

  const statCards = () => [
    {
      label: '机器人总数',
      value: summary()?.bots.total ?? '-',
      icon: (
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 6A2.25 2.25 0 016 3.75h2.25A2.25 2.25 0 0110.5 6v2.25a2.25 2.25 0 01-2.25 2.25H6a2.25 2.25 0 01-2.25-2.25V6zM3.75 15.75A2.25 2.25 0 016 13.5h2.25a2.25 2.25 0 012.25 2.25V18a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 18v-2.25zM13.5 6a2.25 2.25 0 012.25-2.25H18A2.25 2.25 0 0120.25 6v2.25A2.25 2.25 0 0118 10.5h-2.25a2.25 2.25 0 01-2.25-2.25V6zM13.5 15.75a2.25 2.25 0 012.25-2.25H18a2.25 2.25 0 012.25 2.25V18A2.25 2.25 0 0118 20.25h-2.25A2.25 2.25 0 0113.5 18v-2.25z" />
        </svg>
      ),
      color: 'indigo',
    },
    {
      label: '运行中',
      value: summary()?.bots.running ?? '-',
      icon: (
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
      ),
      color: 'emerald',
    },
    {
      label: '总交易数',
      value: summary()?.trades.total ?? '-',
      icon: (
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
        </svg>
      ),
      color: 'amber',
    },
    {
      label: '总盈亏',
      value: summary()?.trades.total_pnl !== undefined
        ? `${summary()!.trades.total_pnl >= 0 ? '+' : ''}${summary()!.trades.total_pnl.toFixed(2)}`
        : '-',
      icon: (
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v12m-3-2.818l.879.659c1.171.879 3.07.879 4.242 0 1.172-.879 1.172-2.303 0-3.182C13.536 12.219 12.768 12 12 12c-.725 0-1.45-.22-2.003-.659-1.106-.879-1.106-2.303 0-3.182s2.9-.879 4.006 0l.415.33M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
      ),
      color: 'violet',
    },
  ]

  const colorClasses: Record<string, { bg: string; icon: string }> = {
    indigo: { bg: 'bg-indigo-50', icon: 'text-indigo-500' },
    emerald: { bg: 'bg-emerald-50', icon: 'text-emerald-500' },
    amber: { bg: 'bg-amber-50', icon: 'text-amber-500' },
    violet: { bg: 'bg-violet-50', icon: 'text-violet-500' },
  }

  return (
    <div>
      <Show when={notifications().length > 0}>
        <div class="fixed top-4 right-4 z-50 space-y-2 max-w-sm">
          <For each={notifications()}>
            {(notif) => (
              <div class={`px-4 py-3 rounded-lg shadow-lg border text-sm animate-slide-in ${
                notif.level === 'error'
                  ? 'bg-red-50 border-red-200 text-red-700'
                  : notif.level === 'warning'
                  ? 'bg-amber-50 border-amber-200 text-amber-700'
                  : 'bg-indigo-50 border-indigo-200 text-indigo-700'
              }`}>
                {notif.message}
              </div>
            )}
          </For>
        </div>
      </Show>

      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
        <For each={statCards()}>
          {(card) => {
            const colors = () => colorClasses[card.color] || colorClasses.indigo
            return (
              <div class="bg-white rounded-xl border border-gray-200/60 p-5">
                <div class="flex items-center justify-between">
                  <div>
                    <p class="text-[13px] text-gray-400">{card.label}</p>
                    <p class="text-2xl font-semibold text-gray-900 mt-1">{card.value}</p>
                  </div>
                  <div class={`p-2.5 rounded-lg ${colors().bg} ${colors().icon}`}>
                    {card.icon}
                  </div>
                </div>
              </div>
            )
          }}
        </For>
      </div>

      <div class="bg-white rounded-xl border border-gray-200/60">
        <div class="px-6 py-4 border-b border-gray-100">
          <h2 class="text-base font-semibold text-gray-900">最近交易</h2>
        </div>

        <Show
          when={!loading() && !error()}
          fallback={
            <div class="p-6 text-center">
              <Show when={loading()}>
                <div class="flex items-center justify-center gap-2 text-gray-400">
                  <svg class="animate-spin w-5 h-5" fill="none" viewBox="0 0 24 24">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                  </svg>
                  <span class="text-sm">加载中...</span>
                </div>
              </Show>
              <Show when={error()}>
                <p class="text-sm text-red-500">{error()}</p>
              </Show>
            </div>
          }
        >
          <Show
            when={trades().length > 0}
            fallback={
              <div class="p-6 text-center text-sm text-gray-400">
                暂无交易记录
              </div>
            }
          >
            <div class="overflow-x-auto">
              <table class="w-full">
                <thead>
                  <tr>
                    <th class="px-6 py-3 text-left text-[13px] font-medium text-gray-400 uppercase tracking-wider">
                      时间
                    </th>
                    <th class="px-6 py-3 text-left text-[13px] font-medium text-gray-400 uppercase tracking-wider">
                      标的
                    </th>
                    <th class="px-6 py-3 text-left text-[13px] font-medium text-gray-400 uppercase tracking-wider">
                      方向
                    </th>
                    <th class="px-6 py-3 text-right text-[13px] font-medium text-gray-400 uppercase tracking-wider">
                      网格层
                    </th>
                    <th class="px-6 py-3 text-right text-[13px] font-medium text-gray-400 uppercase tracking-wider">
                      数量
                    </th>
                    <th class="px-6 py-3 text-right text-[13px] font-medium text-gray-400 uppercase tracking-wider">
                      价格
                    </th>
                    <th class="px-6 py-3 text-right text-[13px] font-medium text-gray-400 uppercase tracking-wider">
                      盈亏
                    </th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-gray-100">
                  <For each={trades()}>
                    {(trade) => (
                      <tr class="hover:bg-gray-50/50 transition-colors">
                        <td class="px-6 py-3.5 text-sm text-gray-500 whitespace-nowrap">
                          {new Date(trade.created_at).toLocaleString('zh-CN')}
                        </td>
                        <td class="px-6 py-3.5 text-sm font-medium text-gray-900">
                          {trade.symbol}
                        </td>
                        <td class="px-6 py-3.5">
                          <span
                            class={`inline-flex items-center px-2 py-0.5 rounded-md text-xs font-medium ${
                              trade.side === 'buy'
                                ? 'bg-emerald-50 text-emerald-600'
                                : 'bg-rose-50 text-rose-600'
                            }`}
                          >
                            {trade.side === 'buy' ? '买入' : '卖出'}
                          </span>
                        </td>
                        <td class="px-6 py-3.5 text-sm text-gray-900 text-right tabular-nums">
                          {trade.grid_level}
                        </td>
                        <td class="px-6 py-3.5 text-sm text-gray-900 text-right tabular-nums">
                          {trade.quantity}
                        </td>
                        <td class="px-6 py-3.5 text-sm text-gray-900 text-right tabular-nums">
                          {trade.price.toFixed(2)}
                        </td>
                        <td class={`px-6 py-3.5 text-sm text-right tabular-nums font-medium ${
                          trade.pnl >= 0 ? 'text-emerald-600' : 'text-rose-600'
                        }`}>
                          {trade.pnl >= 0 ? '+' : ''}{trade.pnl.toFixed(2)}
                        </td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  )
}

export default Dashboard
