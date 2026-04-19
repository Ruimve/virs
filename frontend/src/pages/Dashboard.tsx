import { type Component, createSignal, createEffect, Show, For } from 'solid-js'
import { api } from '../lib/api'

interface DashboardSummary {
  total_strategies: number
  active_strategies: number
  open_positions: number
  today_trades: number
}

interface Trade {
  id: number
  strategy_id?: number
  strategy_name?: string
  symbol: string
  side: 'buy' | 'sell'
  quantity: number
  price: number
  status: string
  created_at: string
}

const Dashboard: Component = () => {
  const [summary, setSummary] = createSignal<DashboardSummary | null>(null)
  const [trades, setTrades] = createSignal<Trade[]>([])
  const [loading, setLoading] = createSignal(true)
  const [error, setError] = createSignal('')

  createEffect(() => {
    loadDashboard()
  })

  async function loadDashboard() {
    setLoading(true)
    setError('')

    try {
      const [summaryResult, tradesResult] = await Promise.all([
        api.get<DashboardSummary>('/dashboard/summary'),
        api.get<{ items: Trade[] }>('/trades?page=1&page_size=10'),
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
      label: '策略总数',
      value: summary()?.total_strategies ?? '-',
      icon: (
        <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
        </svg>
      ),
      color: 'blue',
    },
    {
      label: '活跃策略',
      value: summary()?.active_strategies ?? '-',
      icon: (
        <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
      ),
      color: 'green',
    },
    {
      label: '持仓数',
      value: summary()?.open_positions ?? '-',
      icon: (
        <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M3 3h2l.4 2M7 13h10l4-8H5.4M7 13L5.4 5M7 13l-2.293 2.293c-.63.63-.184 1.707.707 1.707H17m0 0a2 2 0 100 4 2 2 0 000-4zm-8 2a2 2 0 100 4 2 2 0 000-4z" />
        </svg>
      ),
      color: 'yellow',
    },
    {
      label: '今日交易',
      value: summary()?.today_trades ?? '-',
      icon: (
        <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
        </svg>
      ),
      color: 'purple',
    },
  ]

  const colorClasses: Record<string, { bg: string; icon: string; text: string }> = {
    blue: { bg: 'bg-blue-50', icon: 'text-blue-600', text: 'text-blue-700' },
    green: { bg: 'bg-green-50', icon: 'text-green-600', text: 'text-green-700' },
    yellow: { bg: 'bg-yellow-50', icon: 'text-yellow-600', text: 'text-yellow-700' },
    purple: { bg: 'bg-purple-50', icon: 'text-purple-600', text: 'text-purple-700' },
  }

  return (
    <div>
      {/* 统计卡片 */}
      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
        <For each={statCards()}>
          {(card) => {
            const colors = () => colorClasses[card.color] || colorClasses.blue
            return (
              <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
                <div class="flex items-center justify-between">
                  <div>
                    <p class="text-sm font-medium text-gray-500">{card.label}</p>
                    <p class={`text-2xl font-bold mt-1 ${colors().text}`}>{card.value}</p>
                  </div>
                  <div class={`p-3 rounded-lg ${colors().bg} ${colors().icon}`}>
                    {card.icon}
                  </div>
                </div>
              </div>
            )
          }}
        </For>
      </div>

      {/* 最近交易列表 */}
      <div class="bg-white rounded-xl shadow-sm border border-gray-200">
        <div class="px-6 py-4 border-b border-gray-200">
          <h2 class="text-lg font-semibold text-gray-800">最近交易</h2>
        </div>

        <Show
          when={!loading() && !error()}
          fallback={
            <div class="p-6 text-center">
              <Show when={loading()}>
                <div class="flex items-center justify-center gap-2 text-gray-500">
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
              <div class="p-6 text-center text-sm text-gray-500">
                暂无交易记录
              </div>
            }
          >
            <div class="overflow-x-auto">
              <table class="w-full">
                <thead>
                  <tr class="bg-gray-50">
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      时间
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      策略
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      标的
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      方向
                    </th>
                    <th class="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">
                      数量
                    </th>
                    <th class="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">
                      价格
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      状态
                    </th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-gray-200">
                  <For each={trades()}>
                    {(trade) => (
                      <tr class="hover:bg-gray-50 transition-colors">
                        <td class="px-6 py-4 text-sm text-gray-600 whitespace-nowrap">
                          {new Date(trade.created_at).toLocaleString('zh-CN')}
                        </td>
                        <td class="px-6 py-4 text-sm text-gray-900">
                          {trade.strategy_name || `#${trade.strategy_id || '-'}`}
                        </td>
                        <td class="px-6 py-4 text-sm font-medium text-gray-900">
                          {trade.symbol}
                        </td>
                        <td class="px-6 py-4">
                          <span
                            class={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                              trade.side === 'buy'
                                ? 'bg-green-100 text-green-800'
                                : 'bg-red-100 text-red-800'
                            }`}
                          >
                            {trade.side === 'buy' ? '买入' : '卖出'}
                          </span>
                        </td>
                        <td class="px-6 py-4 text-sm text-gray-900 text-right">
                          {trade.quantity}
                        </td>
                        <td class="px-6 py-4 text-sm text-gray-900 text-right">
                          {trade.price.toFixed(2)}
                        </td>
                        <td class="px-6 py-4">
                          <span
                            class={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                              trade.status === 'filled'
                                ? 'bg-green-100 text-green-800'
                                : trade.status === 'pending'
                                ? 'bg-yellow-100 text-yellow-800'
                                : 'bg-gray-100 text-gray-800'
                            }`}
                          >
                            {trade.status === 'filled'
                              ? '已成交'
                              : trade.status === 'pending'
                              ? '待成交'
                              : trade.status}
                          </span>
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
