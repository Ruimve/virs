import { type Component, createSignal, createEffect, Show, For, onMount } from 'solid-js'
import { api } from '../lib/api'
import { useWs, type WsEvent } from '../lib/ws'

interface Trade {
  id: string
  bot_id: string
  symbol: string
  grid_level: number
  open_side: string
  open_price: number
  open_quantity: number
  open_order_id: string | null
  opened_at: string
  close_side: string | null
  close_price: number | null
  close_quantity: number | null
  close_order_id: string | null
  closed_at: string | null
  pnl: number
  pnl_pct: number
  status: string
  created_at: string
}

interface GridBot {
  id: string
  name: string
  symbol: string
  exchange: string
  status: string
  upper_price: number
  lower_price: number
  grid_count: number
  total_pnl: number
  total_trades: number
  created_at: string
}

function formatNumber(n: number, decimals = 2): string {
  return n.toFixed(decimals)
}

function formatDateTime(s: string): string {
  if (!s) return '-'
  try {
    const d = new Date(s)
    return d.toLocaleString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    })
  } catch {
    return s
  }
}

type TabKey = 'trades' | 'bots'

const TABS: { key: TabKey; label: string }[] = [
  { key: 'trades', label: '交易记录' },
  { key: 'bots', label: '网格机器人' },
]

const Trades: Component = () => {
  const [activeTab, setActiveTab] = createSignal<TabKey>('trades')

  const [trades, setTrades] = createSignal<Trade[]>([])
  const [tradesLoading, setTradesLoading] = createSignal(false)
  const [tradesError, setTradesError] = createSignal('')

  const [bots, setBots] = createSignal<GridBot[]>([])
  const [botsLoading, setBotsLoading] = createSignal(false)
  const [botsError, setBotsError] = createSignal('')

  async function loadTrades() {
    setTradesLoading(true)
    setTradesError('')
    try {
      const res = await api.get<{ items: Trade[] }>('/trades')
      if (res.success && res.data) {
        setTrades(res.data.items || [])
      } else {
        setTradesError(res.error || '加载交易记录失败')
      }
    } catch (e) {
      setTradesError(e instanceof Error ? e.message : '加载交易记录失败')
    } finally {
      setTradesLoading(false)
    }
  }

  async function loadBots() {
    setBotsLoading(true)
    setBotsError('')
    try {
      const res = await api.get<{ items: GridBot[] }>('/positions')
      if (res.success && res.data) {
        setBots(res.data.items || [])
      } else {
        setBotsError(res.error || '加载机器人列表失败')
      }
    } catch (e) {
      setBotsError(e instanceof Error ? e.message : '加载机器人列表失败')
    } finally {
      setBotsLoading(false)
    }
  }

  createEffect(() => {
    const tab = activeTab()
    if (tab === 'trades') loadTrades()
    else if (tab === 'bots') loadBots()
  })

  onMount(() => {
    loadTrades()
  })

  let wsReloadTimer: ReturnType<typeof setTimeout> | null = null

  useWs((_event: WsEvent) => {
    if (wsReloadTimer) clearTimeout(wsReloadTimer)
    wsReloadTimer = setTimeout(() => {
      if (activeTab() === 'trades') loadTrades()
      else if (activeTab() === 'bots') loadBots()
    }, 500)
  })

  return (
    <div class="space-y-6">
      <div>
        <h2 class="text-lg font-semibold text-gray-900">交易记录</h2>
        <p class="text-sm text-gray-400 mt-1">查看网格交易记录和机器人状态</p>
      </div>

      <div class="flex gap-1 bg-gray-100/80 rounded-lg p-1 w-fit">
        <For each={TABS}>
          {(tab) => (
            <button
              class={`px-4 py-1.5 text-sm rounded-md transition-colors ${
                activeTab() === tab.key
                  ? 'bg-white text-gray-900 shadow-sm font-medium'
                  : 'text-gray-400 hover:text-gray-600'
              }`}
              onClick={() => setActiveTab(tab.key)}
            >
              {tab.label}
            </button>
          )}
        </For>
      </div>

      <Show when={activeTab() === 'trades'}>
        <div class="bg-white rounded-xl border border-gray-200/60 p-5">
          <Show
            when={!tradesLoading()}
            fallback={
              <div class="text-center py-16">
                <div class="inline-block w-5 h-5 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin"></div>
                <p class="text-sm text-gray-400 mt-2">加载中...</p>
              </div>
            }
          >
            <Show
              when={tradesError() === ''}
              fallback={
                <div class="text-center py-16">
                  <p class="text-sm text-red-400">{tradesError()}</p>
                  <button
                    class="mt-2 text-sm text-indigo-600 hover:text-indigo-700"
                    onClick={loadTrades}
                  >
                    重试
                  </button>
                </div>
              }
            >
              <Show
                when={trades().length > 0}
                fallback={
                  <div class="text-center py-16">
                    <svg class="w-12 h-12 mx-auto text-gray-200 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
                    </svg>
                    <p class="text-gray-400 text-sm">暂无交易记录</p>
                    <p class="text-[13px] text-gray-300 mt-1">交易记录将在网格机器人执行后自动生成</p>
                  </div>
                }
              >
                <div class="overflow-x-auto">
                  <table class="w-full text-sm">
                    <thead>
                      <tr class="border-b border-gray-100">
                        <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">时间</th>
                        <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">交易对</th>
                        <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">网格层</th>
                        <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">开仓</th>
                        <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">平仓</th>
                        <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">盈亏</th>
                        <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">状态</th>
                      </tr>
                    </thead>
                    <tbody>
                      <For each={trades()}>
                        {(trade) => (
                          <tr class="border-b border-gray-50 hover:bg-gray-50/50 transition-colors">
                            <td class="py-2.5 px-3 text-gray-600 whitespace-nowrap text-[13px]">
                              {formatDateTime(trade.opened_at)}
                            </td>
                            <td class="py-2.5 px-3 text-gray-800 font-medium">{trade.symbol}</td>
                            <td class="py-2.5 px-3 text-right text-gray-600 text-[13px]">
                              {trade.grid_level}
                            </td>
                            <td class="py-2.5 px-3">
                              <span class="inline-block px-2 py-0.5 rounded-md text-xs font-medium bg-emerald-50 text-emerald-600">
                                {trade.open_side === 'buy' ? '买入' : '卖出'} {formatNumber(trade.open_price)} × {formatNumber(trade.open_quantity, 4)}
                              </span>
                            </td>
                            <td class="py-2.5 px-3">
                              {trade.close_side ? (
                                <span class={`inline-block px-2 py-0.5 rounded-md text-xs font-medium ${trade.close_side === 'buy' ? 'bg-emerald-50 text-emerald-600' : 'bg-red-50 text-red-500'}`}>
                                  {trade.close_side === 'buy' ? '买入' : '卖出'} {formatNumber(trade.close_price!)} × {formatNumber(trade.close_quantity!, 4)}
                                </span>
                              ) : (
                                <span class="text-gray-400 text-xs">-</span>
                              )}
                            </td>
                            <td
                              class={`py-2.5 px-3 text-right font-medium text-[13px] ${
                                trade.pnl >= 0 ? 'text-emerald-600' : 'text-red-500'
                              }`}
                            >
                              {trade.close_side ? `${trade.pnl >= 0 ? '+' : ''}${formatNumber(trade.pnl)}` : '-'}
                            </td>
                            <td class="py-2.5 px-3">
                              <span class={`inline-block px-2 py-0.5 rounded-md text-xs font-medium ${trade.status === 'open' ? 'bg-blue-50 text-blue-600' : trade.status === 'orphaned' ? 'bg-amber-50 text-amber-600' : 'bg-gray-50 text-gray-500'}`}>
                                {trade.status === 'open' ? '持仓中' : trade.status === 'orphaned' ? '异常' : '已平仓'}
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
          </Show>
        </div>
      </Show>

      <Show when={activeTab() === 'bots'}>
        <div>
          <Show
            when={!botsLoading()}
            fallback={
              <div class="bg-white rounded-xl border border-gray-200/60 p-16 text-center">
                <div class="inline-block w-5 h-5 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin"></div>
                <p class="text-sm text-gray-400 mt-2">加载中...</p>
              </div>
            }
          >
            <Show
              when={botsError() === ''}
              fallback={
                <div class="bg-white rounded-xl border border-gray-200/60 p-16 text-center">
                  <p class="text-sm text-red-400">{botsError()}</p>
                  <button
                    class="mt-2 text-sm text-indigo-600 hover:text-indigo-700"
                    onClick={loadBots}
                  >
                    重试
                  </button>
                </div>
              }
            >
              <Show
                when={bots().length > 0}
                fallback={
                  <div class="bg-white rounded-xl border border-gray-200/60 p-16 text-center">
                    <svg class="w-12 h-12 mx-auto text-gray-200 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 6A2.25 2.25 0 016 3.75h2.25A2.25 2.25 0 0110.5 6v2.25a2.25 2.25 0 01-2.25 2.25H6a2.25 2.25 0 01-2.25-2.25V6z" />
                    </svg>
                    <p class="text-gray-400 text-sm">暂无网格机器人</p>
                  </div>
                }
              >
                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                  <For each={bots()}>
                    {(bot) => (
                      <div class="bg-white rounded-xl border border-gray-200/60 p-4">
                        <div class="flex items-center justify-between mb-3">
                          <h4 class="text-[15px] font-semibold text-gray-900">{bot.name}</h4>
                          <span
                            class={`inline-block px-2 py-0.5 rounded-md text-xs font-medium ${
                              bot.status === 'running'
                                ? 'bg-emerald-50 text-emerald-600'
                                : 'bg-gray-50 text-gray-500'
                            }`}
                          >
                            {bot.status === 'running' ? '运行中' : bot.status}
                          </span>
                        </div>

                        <div class="space-y-2 text-sm">
                          <div class="flex justify-between">
                            <span class="text-gray-400 text-[13px]">交易对</span>
                            <span class="text-gray-700 font-medium text-[13px]">{bot.symbol}</span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-400 text-[13px]">交易所</span>
                            <span class="text-gray-600 text-[13px]">{bot.exchange}</span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-400 text-[13px]">价格区间</span>
                            <span class="text-gray-600 text-[13px]">
                              {formatNumber(bot.lower_price)} - {formatNumber(bot.upper_price)}
                            </span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-400 text-[13px]">网格数</span>
                            <span class="text-gray-600 text-[13px]">{bot.grid_count}</span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-400 text-[13px]">总交易</span>
                            <span class="text-gray-600 text-[13px]">{bot.total_trades}</span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-400 text-[13px]">总盈亏</span>
                            <span
                              class={`font-semibold text-[13px] ${
                                bot.total_pnl >= 0 ? 'text-emerald-600' : 'text-red-500'
                              }`}
                            >
                              {bot.total_pnl >= 0 ? '+' : ''}
                              {formatNumber(bot.total_pnl)}
                            </span>
                          </div>
                        </div>

                        <div class="mt-3 pt-3 border-t border-gray-100 text-xs text-gray-300">
                          创建: {formatDateTime(bot.created_at)}
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </Show>
            </Show>
          </Show>
        </div>
      </Show>
    </div>
  )
}

export default Trades
