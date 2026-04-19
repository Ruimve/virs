import { type Component, createSignal, createEffect, Show, For, onMount } from 'solid-js'
import { api, type PaginatedResponse } from '../lib/api'

// ---- 类型定义 ----

interface Trade {
  id: string
  strategy_id: string
  symbol: string
  side: 'buy' | 'sell'
  trade_type: string
  price: number
  amount: number
  fee: number
  pnl: number
  exchange_order_id?: string
  created_at: string
}

interface Position {
  id: string
  strategy_id: string
  symbol: string
  side: 'long' | 'short'
  size: number
  entry_price: number
  current_price: number
  unrealized_pnl: number
  realized_pnl: number
  leverage: number
  stop_loss?: number
  take_profit?: number
  opened_at: string
  closed_at?: string
}

interface PendingOrder {
  id: string
  strategy_id: string
  symbol: string
  signal_type: string
  order_type: string
  side: string
  amount: number
  price?: number
  status: 'pending' | 'dispatched' | 'filled' | 'failed' | 'canceled'
  attempts: number
  max_attempts: number
  error_message?: string
  created_at: string
}

// ---- 工具函数 ----

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

function getStatusBadge(status: string): { label: string; class: string } {
  switch (status) {
    case 'pending':
      return { label: '待处理', class: 'bg-yellow-100 text-yellow-700' }
    case 'dispatched':
      return { label: '已派发', class: 'bg-blue-100 text-blue-700' }
    case 'filled':
      return { label: '已成交', class: 'bg-green-100 text-green-700' }
    case 'failed':
      return { label: '失败', class: 'bg-red-100 text-red-700' }
    case 'canceled':
      return { label: '已取消', class: 'bg-gray-100 text-gray-700' }
    default:
      return { label: status, class: 'bg-gray-100 text-gray-700' }
  }
}

// ---- Tab 类型 ----

type TabKey = 'trades' | 'positions' | 'orders'

const TABS: { key: TabKey; label: string }[] = [
  { key: 'trades', label: '交易记录' },
  { key: 'positions', label: '当前持仓' },
  { key: 'orders', label: '待处理订单' },
]

// ---- 组件 ----

const Trades: Component = () => {
  // Tab 状态
  const [activeTab, setActiveTab] = createSignal<TabKey>('trades')

  // 交易记录状态
  const [trades, setTrades] = createSignal<Trade[]>([])
  const [tradesPage, setTradesPage] = createSignal(1)
  const [tradesTotalPages, setTradesTotalPages] = createSignal(0)
  const [tradesLoading, setTradesLoading] = createSignal(false)
  const [tradesError, setTradesError] = createSignal('')

  // 持仓状态
  const [positions, setPositions] = createSignal<Position[]>([])
  const [positionsLoading, setPositionsLoading] = createSignal(false)
  const [positionsError, setPositionsError] = createSignal('')

  // 待处理订单状态
  const [orders, setOrders] = createSignal<PendingOrder[]>([])
  const [ordersPage, setOrdersPage] = createSignal(1)
  const [ordersTotalPages, setOrdersTotalPages] = createSignal(0)
  const [ordersLoading, setOrdersLoading] = createSignal(false)
  const [ordersError, setOrdersError] = createSignal('')

  // 加载交易记录
  async function loadTrades(page: number) {
    setTradesLoading(true)
    setTradesError('')
    try {
      const res = await api.get<PaginatedResponse<Trade>>(
        `/trades?page=${page}&page_size=20`
      )
      if (res.success && res.data) {
        setTrades(res.data.items)
        setTradesTotalPages(res.data.total_pages)
        setTradesPage(res.data.page)
      } else {
        setTradesError(res.error || '加载交易记录失败')
      }
    } catch (e) {
      setTradesError(e instanceof Error ? e.message : '加载交易记录失败')
    } finally {
      setTradesLoading(false)
    }
  }

  // 加载持仓
  async function loadPositions() {
    setPositionsLoading(true)
    setPositionsError('')
    try {
      const res = await api.get<Position[]>('/positions')
      if (res.success && res.data) {
        setPositions(res.data)
      } else {
        setPositionsError(res.error || '加载持仓失败')
      }
    } catch (e) {
      setPositionsError(e instanceof Error ? e.message : '加载持仓失败')
    } finally {
      setPositionsLoading(false)
    }
  }

  // 加载待处理订单
  async function loadOrders(page: number) {
    setOrdersLoading(true)
    setOrdersError('')
    try {
      const res = await api.get<PaginatedResponse<PendingOrder>>(
        `/pending-orders?page=${page}&page_size=20`
      )
      if (res.success && res.data) {
        setOrders(res.data.items)
        setOrdersTotalPages(res.data.total_pages)
        setOrdersPage(res.data.page)
      } else {
        setOrdersError(res.error || '加载待处理订单失败')
      }
    } catch (e) {
      setOrdersError(e instanceof Error ? e.message : '加载待处理订单失败')
    } finally {
      setOrdersLoading(false)
    }
  }

  // Tab 切换时加载数据
  createEffect(() => {
    const tab = activeTab()
    if (tab === 'trades') {
      loadTrades(tradesPage())
    } else if (tab === 'positions') {
      loadPositions()
    } else if (tab === 'orders') {
      loadOrders(ordersPage())
    }
  })

  onMount(() => {
    loadTrades(1)
  })

  return (
    <div class="space-y-6">
      {/* 页头 */}
      <div>
        <h2 class="text-lg font-semibold text-gray-800">交易记录</h2>
        <p class="text-sm text-gray-500 mt-1">查看所有交易执行记录、当前持仓和待处理订单</p>
      </div>

      {/* Tab 切换 */}
      <div class="border-b border-gray-200">
        <nav class="flex gap-0">
          <For each={TABS}>
            {(tab) => (
              <button
                class={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors ${
                  activeTab() === tab.key
                    ? 'border-blue-600 text-blue-600'
                    : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300'
                }`}
                onClick={() => setActiveTab(tab.key)}
              >
                {tab.label}
              </button>
            )}
          </For>
        </nav>
      </div>

      {/* 交易记录 Tab */}
      <Show when={activeTab() === 'trades'}>
        <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
          <Show
            when={!tradesLoading()}
            fallback={
              <div class="text-center py-12">
                <div class="inline-block w-6 h-6 border-2 border-blue-600 border-t-transparent rounded-full animate-spin"></div>
                <p class="text-sm text-gray-500 mt-2">加载中...</p>
              </div>
            }
          >
            <Show
              when={tradesError() === ''}
              fallback={
                <div class="text-center py-12">
                  <p class="text-sm text-red-500">{tradesError()}</p>
                  <button
                    class="mt-2 text-sm text-blue-600 hover:underline"
                    onClick={() => loadTrades(tradesPage())}
                  >
                    重试
                  </button>
                </div>
              }
            >
              <Show
                when={trades().length > 0}
                fallback={
                  <div class="text-center py-12">
                    <svg class="w-16 h-16 mx-auto text-gray-300 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
                    </svg>
                    <p class="text-gray-500">暂无交易记录</p>
                    <p class="text-sm text-gray-400 mt-1">交易记录将在策略执行后自动生成</p>
                  </div>
                }
              >
                <div class="overflow-x-auto">
                  <table class="w-full text-sm">
                    <thead>
                      <tr class="border-b border-gray-200">
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">时间</th>
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">策略ID</th>
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">交易对</th>
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">方向</th>
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">类型</th>
                        <th class="text-right py-2 px-3 text-gray-500 font-medium">价格</th>
                        <th class="text-right py-2 px-3 text-gray-500 font-medium">数量</th>
                        <th class="text-right py-2 px-3 text-gray-500 font-medium">手续费</th>
                        <th class="text-right py-2 px-3 text-gray-500 font-medium">盈亏</th>
                      </tr>
                    </thead>
                    <tbody>
                      <For each={trades()}>
                        {(trade) => (
                          <tr class="border-b border-gray-100 hover:bg-gray-50">
                            <td class="py-2 px-3 text-gray-700 whitespace-nowrap">
                              {formatDateTime(trade.created_at)}
                            </td>
                            <td class="py-2 px-3 text-gray-700 font-mono text-xs">
                              {trade.strategy_id.length > 8
                                ? `${trade.strategy_id.slice(0, 8)}...`
                                : trade.strategy_id}
                            </td>
                            <td class="py-2 px-3 text-gray-700 font-medium">{trade.symbol}</td>
                            <td class="py-2 px-3">
                              <span
                                class={`inline-block px-2 py-0.5 rounded text-xs font-medium ${
                                  trade.side === 'buy'
                                    ? 'bg-green-100 text-green-700'
                                    : 'bg-red-100 text-red-700'
                                }`}
                              >
                                {trade.side === 'buy' ? '买入' : '卖出'}
                              </span>
                            </td>
                            <td class="py-2 px-3 text-gray-700">{trade.trade_type}</td>
                            <td class="py-2 px-3 text-right text-gray-700">
                              {formatNumber(trade.price)}
                            </td>
                            <td class="py-2 px-3 text-right text-gray-700">
                              {formatNumber(trade.amount, 4)}
                            </td>
                            <td class="py-2 px-3 text-right text-gray-500">
                              {formatNumber(trade.fee)}
                            </td>
                            <td
                              class={`py-2 px-3 text-right font-medium ${
                                trade.pnl >= 0 ? 'text-green-600' : 'text-red-600'
                              }`}
                            >
                              {trade.pnl >= 0 ? '+' : ''}
                              {formatNumber(trade.pnl)}
                            </td>
                          </tr>
                        )}
                      </For>
                    </tbody>
                  </table>
                </div>

                {/* 分页 */}
                <Show when={tradesTotalPages() > 1}>
                  <div class="flex items-center justify-between mt-4 pt-4 border-t border-gray-200">
                    <p class="text-sm text-gray-500">
                      第 {tradesPage()} / {tradesTotalPages()} 页
                    </p>
                    <div class="flex gap-2">
                      <button
                        class="px-3 py-1.5 text-sm border border-gray-300 rounded-lg hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
                        disabled={tradesPage() <= 1}
                        onClick={() => loadTrades(tradesPage() - 1)}
                      >
                        上一页
                      </button>
                      <button
                        class="px-3 py-1.5 text-sm border border-gray-300 rounded-lg hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
                        disabled={tradesPage() >= tradesTotalPages()}
                        onClick={() => loadTrades(tradesPage() + 1)}
                      >
                        下一页
                      </button>
                    </div>
                  </div>
                </Show>
              </Show>
            </Show>
          </Show>
        </div>
      </Show>

      {/* 当前持仓 Tab */}
      <Show when={activeTab() === 'positions'}>
        <div>
          <Show
            when={!positionsLoading()}
            fallback={
              <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-12 text-center">
                <div class="inline-block w-6 h-6 border-2 border-blue-600 border-t-transparent rounded-full animate-spin"></div>
                <p class="text-sm text-gray-500 mt-2">加载中...</p>
              </div>
            }
          >
            <Show
              when={positionsError() === ''}
              fallback={
                <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-12 text-center">
                  <p class="text-sm text-red-500">{positionsError()}</p>
                  <button
                    class="mt-2 text-sm text-blue-600 hover:underline"
                    onClick={loadPositions}
                  >
                    重试
                  </button>
                </div>
              }
            >
              <Show
                when={positions().length > 0}
                fallback={
                  <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-12 text-center">
                    <svg class="w-16 h-16 mx-auto text-gray-300 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" />
                    </svg>
                    <p class="text-gray-500">当前无持仓</p>
                  </div>
                }
              >
                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                  <For each={positions()}>
                    {(pos) => (
                      <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
                        {/* 头部: 交易对 + 方向 */}
                        <div class="flex items-center justify-between mb-3">
                          <h4 class="text-base font-semibold text-gray-800">{pos.symbol}</h4>
                          <span
                            class={`inline-block px-2 py-0.5 rounded text-xs font-medium ${
                              pos.side === 'long'
                                ? 'bg-green-100 text-green-700'
                                : 'bg-red-100 text-red-700'
                            }`}
                          >
                            {pos.side === 'long' ? '做多' : '做空'}
                          </span>
                        </div>

                        {/* 价格信息 */}
                        <div class="space-y-2 text-sm">
                          <div class="flex justify-between">
                            <span class="text-gray-500">数量</span>
                            <span class="text-gray-700 font-medium">
                              {formatNumber(pos.size, 4)}
                            </span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-500">入场价</span>
                            <span class="text-gray-700">
                              {formatNumber(pos.entry_price)}
                            </span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-500">当前价</span>
                            <span class="text-gray-700">
                              {formatNumber(pos.current_price)}
                            </span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-500">未实现盈亏</span>
                            <span
                              class={`font-semibold ${
                                pos.unrealized_pnl >= 0 ? 'text-green-600' : 'text-red-600'
                              }`}
                            >
                              {pos.unrealized_pnl >= 0 ? '+' : ''}
                              {formatNumber(pos.unrealized_pnl)}
                            </span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-500">杠杆</span>
                            <span class="text-gray-700">{pos.leverage}x</span>
                          </div>

                          {/* 止损/止盈 */}
                          <Show when={pos.stop_loss != null || pos.take_profit != null}>
                            <div class="border-t border-gray-100 pt-2 mt-2 space-y-1">
                              <Show when={pos.stop_loss != null}>
                                <div class="flex justify-between">
                                  <span class="text-gray-500">止损</span>
                                  <span class="text-red-500">
                                    {formatNumber(pos.stop_loss!)}
                                  </span>
                                </div>
                              </Show>
                              <Show when={pos.take_profit != null}>
                                <div class="flex justify-between">
                                  <span class="text-gray-500">止盈</span>
                                  <span class="text-green-600">
                                    {formatNumber(pos.take_profit!)}
                                  </span>
                                </div>
                              </Show>
                            </div>
                          </Show>
                        </div>

                        {/* 底部: 开仓时间 */}
                        <div class="mt-3 pt-3 border-t border-gray-100 text-xs text-gray-400">
                          开仓: {formatDateTime(pos.opened_at)}
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

      {/* 待处理订单 Tab */}
      <Show when={activeTab() === 'orders'}>
        <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
          <Show
            when={!ordersLoading()}
            fallback={
              <div class="text-center py-12">
                <div class="inline-block w-6 h-6 border-2 border-blue-600 border-t-transparent rounded-full animate-spin"></div>
                <p class="text-sm text-gray-500 mt-2">加载中...</p>
              </div>
            }
          >
            <Show
              when={ordersError() === ''}
              fallback={
                <div class="text-center py-12">
                  <p class="text-sm text-red-500">{ordersError()}</p>
                  <button
                    class="mt-2 text-sm text-blue-600 hover:underline"
                    onClick={() => loadOrders(ordersPage())}
                  >
                    重试
                  </button>
                </div>
              }
            >
              <Show
                when={orders().length > 0}
                fallback={
                  <div class="text-center py-12">
                    <svg class="w-16 h-16 mx-auto text-gray-300 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <p class="text-gray-500">暂无待处理订单</p>
                  </div>
                }
              >
                <div class="overflow-x-auto">
                  <table class="w-full text-sm">
                    <thead>
                      <tr class="border-b border-gray-200">
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">时间</th>
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">策略ID</th>
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">交易对</th>
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">信号类型</th>
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">订单类型</th>
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">方向</th>
                        <th class="text-right py-2 px-3 text-gray-500 font-medium">数量</th>
                        <th class="text-right py-2 px-3 text-gray-500 font-medium">价格</th>
                        <th class="text-center py-2 px-3 text-gray-500 font-medium">状态</th>
                        <th class="text-center py-2 px-3 text-gray-500 font-medium">重试</th>
                        <th class="text-left py-2 px-3 text-gray-500 font-medium">错误信息</th>
                      </tr>
                    </thead>
                    <tbody>
                      <For each={orders()}>
                        {(order) => {
                          const statusBadge = getStatusBadge(order.status)
                          return (
                            <tr class="border-b border-gray-100 hover:bg-gray-50">
                              <td class="py-2 px-3 text-gray-700 whitespace-nowrap">
                                {formatDateTime(order.created_at)}
                              </td>
                              <td class="py-2 px-3 text-gray-700 font-mono text-xs">
                                {order.strategy_id.length > 8
                                  ? `${order.strategy_id.slice(0, 8)}...`
                                  : order.strategy_id}
                              </td>
                              <td class="py-2 px-3 text-gray-700 font-medium">
                                {order.symbol}
                              </td>
                              <td class="py-2 px-3 text-gray-700">{order.signal_type}</td>
                              <td class="py-2 px-3 text-gray-700">{order.order_type}</td>
                              <td class="py-2 px-3 text-gray-700">{order.side}</td>
                              <td class="py-2 px-3 text-right text-gray-700">
                                {formatNumber(order.amount, 4)}
                              </td>
                              <td class="py-2 px-3 text-right text-gray-700">
                                {order.price != null ? formatNumber(order.price) : '-'}
                              </td>
                              <td class="py-2 px-3 text-center">
                                <span
                                  class={`inline-block px-2 py-0.5 rounded text-xs font-medium ${statusBadge.class}`}
                                >
                                  {statusBadge.label}
                                </span>
                              </td>
                              <td class="py-2 px-3 text-center text-gray-700">
                                {order.attempts}/{order.max_attempts}
                              </td>
                              <td class="py-2 px-3 text-gray-500 max-w-[200px] truncate">
                                {order.error_message || '-'}
                              </td>
                            </tr>
                          )
                        }}
                      </For>
                    </tbody>
                  </table>
                </div>

                {/* 分页 */}
                <Show when={ordersTotalPages() > 1}>
                  <div class="flex items-center justify-between mt-4 pt-4 border-t border-gray-200">
                    <p class="text-sm text-gray-500">
                      第 {ordersPage()} / {ordersTotalPages()} 页
                    </p>
                    <div class="flex gap-2">
                      <button
                        class="px-3 py-1.5 text-sm border border-gray-300 rounded-lg hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
                        disabled={ordersPage() <= 1}
                        onClick={() => loadOrders(ordersPage() - 1)}
                      >
                        上一页
                      </button>
                      <button
                        class="px-3 py-1.5 text-sm border border-gray-300 rounded-lg hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
                        disabled={ordersPage() >= ordersTotalPages()}
                        onClick={() => loadOrders(ordersPage() + 1)}
                      >
                        下一页
                      </button>
                    </div>
                  </div>
                </Show>
              </Show>
            </Show>
          </Show>
        </div>
      </Show>
    </div>
  )
}

export default Trades
