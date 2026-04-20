import { type Component, createSignal, createEffect, Show, For } from 'solid-js'
import { api } from '../lib/api'
import KlineChart from '../components/KlineChart'

// ── 类型定义 ──────────────────────────────────────────────
interface TickerData {
  symbol: string
  last_price: number
  price_change: number
  price_change_pct: number
  high_24h: number
  low_24h: number
  volume_24h: number
  [key: string]: unknown
}

interface KlineItem {
  open_time: number
  open: number
  high: number
  low: number
  close: number
  volume: number
  close_time: number
  [key: string]: unknown
}

interface OrderbookEntry {
  price: number
  quantity: number
}

interface OrderbookData {
  bids: OrderbookEntry[]
  asks: OrderbookEntry[]
}

interface BalanceItem {
  currency: string
  available: number
  frozen: number
  total: number
}

// ── 工具函数 ──────────────────────────────────────────────
function formatPrice(v: number | undefined | null, decimals = 2): string {
  if (v === undefined || v === null) return '--'
  return v.toFixed(decimals)
}

function formatVolume(v: number | undefined | null): string {
  if (v === undefined || v === null) return '--'
  if (v >= 1e9) return (v / 1e9).toFixed(2) + 'B'
  if (v >= 1e6) return (v / 1e6).toFixed(2) + 'M'
  if (v >= 1e3) return (v / 1e3).toFixed(2) + 'K'
  return v.toFixed(2)
}

// ── Tab 类型 ──────────────────────────────────────────────

type MarketTab = 'ticker' | 'kline' | 'orderbook' | 'balance'

const MARKET_TABS: { key: MarketTab; label: string }[] = [
  { key: 'ticker', label: 'Ticker' },
  { key: 'kline', label: 'K线' },
  { key: 'orderbook', label: '订单簿' },
  { key: 'balance', label: '余额' },
]

// ── 组件 ──────────────────────────────────────────────────
const Market: Component = () => {
  // 工具栏状态
  const [exchange, setExchange] = createSignal('binance')
  const [symbol, setSymbol] = createSignal('BTCUSDT')
  const [interval, setInterval_] = createSignal('1h')

  // Tab 状态
  const [activeTab, setActiveTab] = createSignal<MarketTab>('ticker')

  // 数据状态
  const [ticker, setTicker] = createSignal<TickerData | null>(null)
  const [tickerLoading, setTickerLoading] = createSignal(false)
  const [tickerError, setTickerError] = createSignal(false)

  const [klines, setKlines] = createSignal<KlineItem[]>([])
  const [klinesLoading, setKlinesLoading] = createSignal(false)
  const [klinesError, setKlinesError] = createSignal(false)

  const [orderbook, setOrderbook] = createSignal<OrderbookData | null>(null)
  const [orderbookLoading, setOrderbookLoading] = createSignal(false)
  const [orderbookError, setOrderbookError] = createSignal(false)

  const [balances, setBalances] = createSignal<BalanceItem[]>([])
  const [_balancesLoading, setBalancesLoading] = createSignal(false)
  const [balancesError, setBalancesError] = createSignal(false)

  // ── 查询行情 ──
  async function fetchTicker() {
    setTickerLoading(true)
    setTickerError(false)
    try {
      const params = new URLSearchParams({
        exchange: exchange(),
        symbol: symbol(),
      })
      const res = await api.get<TickerData>(`/market/ticker?${params}`)
      if (res.success && res.data) {
        setTicker(res.data)
      } else {
        setTicker(null)
        setTickerError(true)
      }
    } catch {
      setTicker(null)
      setTickerError(true)
    } finally {
      setTickerLoading(false)
    }
  }

  // ── 查询K线 ──
  async function fetchKlines() {
    setKlinesLoading(true)
    setKlinesError(false)
    try {
      const params = new URLSearchParams({
        exchange: exchange(),
        symbol: symbol(),
        interval: interval(),
        limit: '100',
      })
      const res = await api.get<KlineItem[]>(`/market/klines?${params}`)
      if (res.success && res.data) {
        setKlines(res.data)
      } else {
        setKlines([])
        setKlinesError(true)
      }
    } catch {
      setKlines([])
      setKlinesError(true)
    } finally {
      setKlinesLoading(false)
    }
  }

  // ── 查询订单簿 ──
  async function fetchOrderbook() {
    setOrderbookLoading(true)
    setOrderbookError(false)
    try {
      const params = new URLSearchParams({
        exchange: exchange(),
        symbol: symbol(),
        depth: '20',
      })
      const res = await api.get<OrderbookData>(`/market/orderbook?${params}`)
      if (res.success && res.data) {
        setOrderbook(res.data)
      } else {
        setOrderbook(null)
        setOrderbookError(true)
      }
    } catch {
      setOrderbook(null)
      setOrderbookError(true)
    } finally {
      setOrderbookLoading(false)
    }
  }

  // ── 查询余额 ──
  async function fetchBalances() {
    setBalancesLoading(true)
    setBalancesError(false)
    try {
      const params = new URLSearchParams({ exchange: exchange() })
      const res = await api.get<BalanceItem[]>(`/market/balances?${params}`)
      if (res.success && res.data) {
        // 仅显示有余额的币种
        const filtered = res.data.filter(
          (b) => (b.available > 0 || b.frozen > 0 || b.total > 0)
        )
        setBalances(filtered)
      } else {
        setBalances([])
        setBalancesError(true)
      }
    } catch {
      setBalances([])
      setBalancesError(true)
    } finally {
      setBalancesLoading(false)
    }
  }

  // ── 查询全部 ──
  async function fetchAll() {
    await Promise.all([fetchTicker(), fetchKlines(), fetchOrderbook()])
  }

  // ── 查询按钮 ──
  function handleQuery() {
    fetchAll()
  }

  // ── 刷新余额 ──
  function handleRefreshBalances() {
    fetchBalances()
  }

  // ── K线周期变化时重新加载K线 ──
  function handleIntervalChange(val: string) {
    setInterval_(val)
  }

  createEffect(() => {
    // interval 变化时自动重新加载 K 线
    interval()
    fetchKlines()
  })

  // ── 计算订单簿累计 ──
  function computeCumulative(entries: OrderbookEntry[]): { entry: OrderbookEntry; cum: number }[] {
    let cum = 0
    return entries.map((e) => {
      cum += e.quantity
      return { entry: e, cum }
    })
  }

  // ── 渲染 ──
  return (
    <div class="space-y-6">
      {/* 标题 */}
      <div>
        <h2 class="text-lg font-semibold text-gray-900">行情查看</h2>
        <p class="text-sm text-gray-400 mt-1">实时市场行情数据</p>
      </div>

      {/* 工具栏 */}
      <div class="bg-white rounded-xl border border-gray-200/60 p-4">
        <div class="flex flex-wrap items-end gap-4">
          {/* 交易所 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">交易所</label>
            <select
              class="px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 bg-white"
              value={exchange()}
              onChange={(e) => setExchange((e.target as HTMLSelectElement).value)}
            >
              <option value="binance">Binance</option>
              <option value="okx">OKX</option>
              <option value="bybit">Bybit</option>
            </select>
          </div>

          {/* 交易对 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">交易对</label>
            <input
              type="text"
              class="w-36 px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 placeholder:text-gray-300"
              value={symbol()}
              onInput={(e) => setSymbol((e.target as HTMLInputElement).value)}
              placeholder="BTCUSDT"
            />
          </div>

          {/* K线周期 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">K线周期</label>
            <select
              class="px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 bg-white"
              value={interval()}
              onChange={(e) => handleIntervalChange((e.target as HTMLSelectElement).value)}
            >
              <option value="1m">1m</option>
              <option value="5m">5m</option>
              <option value="15m">15m</option>
              <option value="1h">1h</option>
              <option value="4h">4h</option>
              <option value="1d">1d</option>
            </select>
          </div>

          {/* 查询按钮 */}
          <button
            class="px-4 py-2 bg-indigo-600 text-white text-sm font-medium rounded-lg hover:bg-indigo-700 transition-colors"
            onClick={handleQuery}
          >
            查询
          </button>

          {/* 刷新余额 */}
          <button
            class="px-4 py-2 bg-gray-50 text-gray-600 text-sm font-medium rounded-lg hover:bg-gray-100 transition-colors border border-gray-200/60"
            onClick={handleRefreshBalances}
          >
            刷新余额
          </button>
        </div>
      </div>

      {/* Tab 切换 */}
      <div class="flex gap-1 bg-gray-100/80 rounded-lg p-1 w-fit">
        <For each={MARKET_TABS}>
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

      {/* Ticker Tab */}
      <Show when={activeTab() === 'ticker'}>
        <div class="space-y-4">
          {/* 行情信息卡片 */}
          <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-3">
            {/* 最新价 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">最新价</p>
              <Show
                when={!tickerLoading() && !tickerError()}
                fallback={
                  <p class="text-xl font-bold text-gray-300">
                    {tickerLoading() ? '--' : '数据不可用'}
                  </p>
                }
              >
                <p class="text-xl font-bold text-gray-900">
                  {ticker() ? formatPrice(ticker()!.last_price) : '--'}
                </p>
              </Show>
            </div>

            {/* 24h 涨跌 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">24h 涨跌</p>
              <Show
                when={!tickerLoading() && !tickerError()}
                fallback={
                  <p class="text-xl font-bold text-gray-300">
                    {tickerLoading() ? '--' : '数据不可用'}
                  </p>
                }
              >
                <Show
                  when={ticker() && ticker()!.price_change_pct >= 0}
                  fallback={
                    <p class="text-xl font-bold text-red-500">
                      {ticker()
                        ? `${ticker()!.price_change_pct.toFixed(2)}% / ${formatPrice(ticker()!.price_change)}`
                        : '--'}
                    </p>
                  }
                >
                  <p class="text-xl font-bold text-emerald-500">
                    {ticker()
                      ? `+${ticker()!.price_change_pct.toFixed(2)}% / +${formatPrice(ticker()!.price_change)}`
                      : '--'}
                  </p>
                </Show>
              </Show>
            </div>

            {/* 24h 最高 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">24h 最高</p>
              <Show
                when={!tickerLoading() && !tickerError()}
                fallback={
                  <p class="text-xl font-bold text-gray-300">
                    {tickerLoading() ? '--' : '数据不可用'}
                  </p>
                }
              >
                <p class="text-xl font-bold text-gray-900">
                  {ticker() ? formatPrice(ticker()!.high_24h) : '--'}
                </p>
              </Show>
            </div>

            {/* 24h 最低 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">24h 最低</p>
              <Show
                when={!tickerLoading() && !tickerError()}
                fallback={
                  <p class="text-xl font-bold text-gray-300">
                    {tickerLoading() ? '--' : '数据不可用'}
                  </p>
                }
              >
                <p class="text-xl font-bold text-gray-900">
                  {ticker() ? formatPrice(ticker()!.low_24h) : '--'}
                </p>
              </Show>
            </div>

            {/* 24h 成交量 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">24h 成交量</p>
              <Show
                when={!tickerLoading() && !tickerError()}
                fallback={
                  <p class="text-xl font-bold text-gray-300">
                    {tickerLoading() ? '--' : '数据不可用'}
                  </p>
                }
              >
                <p class="text-xl font-bold text-gray-900">
                  {ticker() ? formatVolume(ticker()!.volume_24h) : '--'}
                </p>
              </Show>
            </div>
          </div>
        </div>
      </Show>

      {/* K线图 Tab */}
      <Show when={activeTab() === 'kline'}>
        <div class="bg-white rounded-xl border border-gray-200/60 overflow-hidden">
          <div class="p-5">
            <Show
              when={klinesLoading()}
              fallback={
                <Show
                  when={klinesError()}
                  fallback={
                    <Show
                      when={klines().length > 0}
                      fallback={
                        <div class="text-center py-16">
                          <svg class="w-12 h-12 mx-auto text-gray-200 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z" />
                          </svg>
                          <p class="text-gray-400 text-sm">暂无K线数据</p>
                        </div>
                      }
                    >
                      <KlineChart
                        data={klines().map(k => ({
                          time: Math.floor(k.open_time / 1000),
                          open: k.open,
                          high: k.high,
                          low: k.low,
                          close: k.close,
                          volume: k.volume,
                        }))}
                        height={450}
                      />
                    </Show>
                  }
                >
                  <div class="text-center py-16">
                    <p class="text-red-400 text-sm">加载K线数据失败</p>
                  </div>
                </Show>
              }
            >
              <div class="text-center py-16">
                <p class="text-gray-400 text-sm">加载K线数据...</p>
              </div>
            </Show>
          </div>
        </div>
      </Show>

      {/* 订单簿 Tab */}
      <Show when={activeTab() === 'orderbook'}>
        <div class="bg-white rounded-xl border border-gray-200/60 overflow-hidden">
          <div class="p-5">
            <Show
              when={orderbookLoading()}
              fallback={
                <Show
                  when={orderbookError()}
                  fallback={
                    <Show
                      when={orderbook() !== null}
                      fallback={
                        <div class="text-center py-16">
                          <svg class="w-12 h-12 mx-auto text-gray-200 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M3 4h13M3 8h9m-9 4h6m4 0l4-4m0 0l4 4m-4-4v12" />
                          </svg>
                          <p class="text-gray-400 text-sm">暂无订单簿数据</p>
                        </div>
                      }
                    >
                      <div class="grid grid-cols-2 gap-8">
                        {/* 买盘 (bids) */}
                        <div>
                          <div class="grid grid-cols-3 text-[13px] font-medium text-gray-400 mb-2 px-2">
                            <span>价格</span>
                            <span class="text-right">数量</span>
                            <span class="text-right">累计</span>
                          </div>
                          <For each={computeCumulative(orderbook()!.bids)}>
                            {({ entry, cum }) => {
                              const maxCum = computeCumulative(orderbook()!.bids).slice(-1)[0]?.cum || 1
                              const pct = (cum / maxCum) * 100
                              return (
                                <div class="relative grid grid-cols-3 text-xs py-1 px-2">
                                  <div
                                    class="absolute inset-0 bg-emerald-50/60 rounded"
                                    style={{ width: `${pct}%` }}
                                  />
                                  <span class="relative text-emerald-600 font-mono">
                                    {formatPrice(entry.price)}
                                  </span>
                                  <span class="relative text-right text-gray-600 font-mono">
                                    {entry.quantity.toFixed(4)}
                                  </span>
                                  <span class="relative text-right text-gray-400 font-mono">
                                    {cum.toFixed(4)}
                                  </span>
                                </div>
                              )
                            }}
                          </For>
                        </div>

                        {/* 卖盘 (asks) */}
                        <div>
                          <div class="grid grid-cols-3 text-[13px] font-medium text-gray-400 mb-2 px-2">
                            <span>价格</span>
                            <span class="text-right">数量</span>
                            <span class="text-right">累计</span>
                          </div>
                          <For each={computeCumulative(orderbook()!.asks)}>
                            {({ entry, cum }) => {
                              const maxCum = computeCumulative(orderbook()!.asks).slice(-1)[0]?.cum || 1
                              const pct = (cum / maxCum) * 100
                              return (
                                <div class="relative grid grid-cols-3 text-xs py-1 px-2">
                                  <div
                                    class="absolute inset-y-0 right-0 bg-red-50/60 rounded"
                                    style={{ width: `${pct}%` }}
                                  />
                                  <span class="relative text-red-500 font-mono">
                                    {formatPrice(entry.price)}
                                  </span>
                                  <span class="relative text-right text-gray-600 font-mono">
                                    {entry.quantity.toFixed(4)}
                                  </span>
                                  <span class="relative text-right text-gray-400 font-mono">
                                    {cum.toFixed(4)}
                                  </span>
                                </div>
                              )
                            }}
                          </For>
                        </div>
                      </div>
                    </Show>
                  }
                >
                  <div class="text-center py-16">
                    <p class="text-red-400 text-sm">加载订单簿失败</p>
                  </div>
                </Show>
              }
            >
              <div class="text-center py-16">
                <p class="text-gray-400 text-sm">加载订单簿...</p>
              </div>
            </Show>
          </div>
        </div>
      </Show>

      {/* 余额 Tab */}
      <Show when={activeTab() === 'balance'}>
        <div class="bg-white rounded-xl border border-gray-200/60 overflow-hidden">
          <div class="p-5">
            <Show
              when={!balancesError()}
              fallback={
                <div class="text-center py-16">
                  <p class="text-red-400 text-sm">加载余额失败，请重试</p>
                </div>
              }
            >
              <Show
                when={balances().length > 0}
                fallback={
                  <div class="text-center py-16">
                    <svg class="w-12 h-12 mx-auto text-gray-200 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z" />
                    </svg>
                    <p class="text-gray-400 text-sm">暂无余额数据</p>
                  </div>
                }
              >
                <div class="overflow-x-auto">
                  <table class="w-full text-sm">
                    <thead>
                      <tr class="border-b border-gray-100">
                        <th class="text-left px-4 py-2.5 text-[13px] font-medium text-gray-400">币种</th>
                        <th class="text-right px-4 py-2.5 text-[13px] font-medium text-gray-400">可用余额</th>
                        <th class="text-right px-4 py-2.5 text-[13px] font-medium text-gray-400">冻结余额</th>
                        <th class="text-right px-4 py-2.5 text-[13px] font-medium text-gray-400">总余额</th>
                      </tr>
                    </thead>
                    <tbody>
                      <For each={balances()}>
                        {(b) => (
                          <tr class="border-b border-gray-50 hover:bg-gray-50/50 transition-colors">
                            <td class="px-4 py-2.5 font-medium text-gray-800">{b.currency}</td>
                            <td class="px-4 py-2.5 text-right text-gray-500 font-mono text-[13px]">
                              {b.available.toFixed(8)}
                            </td>
                            <td class="px-4 py-2.5 text-right text-gray-500 font-mono text-[13px]">
                              {b.frozen.toFixed(8)}
                            </td>
                            <td class="px-4 py-2.5 text-right text-gray-800 font-mono font-medium text-[13px]">
                              {b.total.toFixed(8)}
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
      </Show>
    </div>
  )
}

export default Market
