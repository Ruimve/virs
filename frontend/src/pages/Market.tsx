import { type Component, createSignal, createEffect, Show, For, onMount } from 'solid-js'
import { api } from '../lib/api'

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

function formatTime(ts: number): string {
  const d = new Date(ts)
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const dd = String(d.getDate()).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const mi = String(d.getMinutes()).padStart(2, '0')
  return `${mm}/${dd} ${hh}:${mi}`
}

// ── 组件 ──────────────────────────────────────────────────
const Market: Component = () => {
  // 工具栏状态
  const [exchange, setExchange] = createSignal('binance')
  const [symbol, setSymbol] = createSignal('BTCUSDT')
  const [interval, setInterval_] = createSignal('1h')

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
  const [balancesLoading, setBalancesLoading] = createSignal(false)
  const [balancesError, setBalancesError] = createSignal(false)
  const [balancesExpanded, setBalancesExpanded] = createSignal(false)

  // Canvas ref
  let chartCanvas!: HTMLCanvasElement

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
        // 等待 DOM 更新后绘制
        requestAnimationFrame(() => drawChart())
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

  // ── Canvas K线图绘制 ──
  function drawChart() {
    const canvas = chartCanvas
    if (!canvas) return

    const data = klines()
    if (!data || data.length === 0) return

    const dpr = window.devicePixelRatio || 1
    const rect = canvas.getBoundingClientRect()
    const width = rect.width
    const height = rect.height

    canvas.width = width * dpr
    canvas.height = height * dpr

    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.scale(dpr, dpr)

    // 边距
    const padding = { top: 20, right: 70, bottom: 30, left: 10 }
    const chartW = width - padding.left - padding.right
    const chartH = height - padding.top - padding.bottom

    // 数据范围
    let minPrice = Infinity
    let maxPrice = -Infinity
    for (const k of data) {
      if (k.low < minPrice) minPrice = k.low
      if (k.high > maxPrice) maxPrice = k.high
    }
    const priceRange = maxPrice - minPrice || 1
    const pricePadding = priceRange * 0.05
    minPrice -= pricePadding
    maxPrice += pricePadding
    const totalRange = maxPrice - minPrice

    // 清空
    ctx.clearRect(0, 0, width, height)
    ctx.fillStyle = '#ffffff'
    ctx.fillRect(0, 0, width, height)

    // 网格线
    ctx.strokeStyle = '#f0f0f0'
    ctx.lineWidth = 1
    const gridLines = 5
    for (let i = 0; i <= gridLines; i++) {
      const y = padding.top + (chartH / gridLines) * i
      ctx.beginPath()
      ctx.moveTo(padding.left, y)
      ctx.lineTo(width - padding.right, y)
      ctx.stroke()

      // Y轴价格标签
      const price = maxPrice - (totalRange / gridLines) * i
      ctx.fillStyle = '#9ca3af'
      ctx.font = '11px monospace'
      ctx.textAlign = 'left'
      ctx.fillText(formatPrice(price, 2), width - padding.right + 8, y + 4)
    }

    // K线蜡烛
    const candleCount = data.length
    const candleWidth = Math.max(1, (chartW / candleCount) * 0.7)
    const gap = chartW / candleCount

    for (let i = 0; i < candleCount; i++) {
      const k = data[i]
      const x = padding.left + gap * i + gap / 2

      const openY = padding.top + ((maxPrice - k.open) / totalRange) * chartH
      const closeY = padding.top + ((maxPrice - k.close) / totalRange) * chartH
      const highY = padding.top + ((maxPrice - k.high) / totalRange) * chartH
      const lowY = padding.top + ((maxPrice - k.low) / totalRange) * chartH

      const isUp = k.close >= k.open
      const color = isUp ? '#22c55e' : '#ef4444'

      // 影线
      ctx.strokeStyle = color
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(x, highY)
      ctx.lineTo(x, lowY)
      ctx.stroke()

      // 实体
      const bodyTop = Math.min(openY, closeY)
      const bodyHeight = Math.max(1, Math.abs(closeY - openY))
      ctx.fillStyle = color
      ctx.fillRect(x - candleWidth / 2, bodyTop, candleWidth, bodyHeight)
    }

    // X轴时间标签 (每隔一定数量显示)
    ctx.fillStyle = '#9ca3af'
    ctx.font = '10px monospace'
    ctx.textAlign = 'center'
    const labelInterval = Math.max(1, Math.floor(candleCount / 8))
    for (let i = 0; i < candleCount; i += labelInterval) {
      const x = padding.left + gap * i + gap / 2
      const y = height - padding.bottom + 18
      ctx.fillText(formatTime(data[i].open_time), x, y)
    }
  }

  // ── 窗口 resize 时重绘 ──
  onMount(() => {
    const handleResize = () => {
      if (klines().length > 0) drawChart()
    }
    window.addEventListener('resize', handleResize)
    return () => window.removeEventListener('resize', handleResize)
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
        <h2 class="text-lg font-semibold text-gray-800">行情查看</h2>
        <p class="text-sm text-gray-500 mt-1">实时市场行情数据</p>
      </div>

      {/* 工具栏 */}
      <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
        <div class="flex flex-wrap items-end gap-4">
          {/* 交易所 */}
          <div>
            <label class="block text-xs font-medium text-gray-500 mb-1">交易所</label>
            <select
              class="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
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
            <label class="block text-xs font-medium text-gray-500 mb-1">交易对</label>
            <input
              type="text"
              class="w-36 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              value={symbol()}
              onInput={(e) => setSymbol((e.target as HTMLInputElement).value)}
              placeholder="BTCUSDT"
            />
          </div>

          {/* K线周期 */}
          <div>
            <label class="block text-xs font-medium text-gray-500 mb-1">K线周期</label>
            <select
              class="px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
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
            class="px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
            onClick={handleQuery}
          >
            查询
          </button>

          {/* 刷新余额 */}
          <button
            class="px-4 py-2 bg-gray-100 text-gray-700 text-sm font-medium rounded-lg hover:bg-gray-200 transition-colors"
            onClick={handleRefreshBalances}
          >
            刷新余额
          </button>
        </div>
      </div>

      {/* 行情信息卡片 */}
      <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-4">
        {/* 最新价 */}
        <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
          <p class="text-xs text-gray-500 mb-1">最新价</p>
          <Show
            when={!tickerLoading() && !tickerError()}
            fallback={
              <p class="text-xl font-bold text-gray-400">
                {tickerLoading() ? '--' : '数据不可用'}
              </p>
            }
          >
            <p class="text-xl font-bold text-gray-800">
              {ticker() ? formatPrice(ticker()!.last_price) : '--'}
            </p>
          </Show>
        </div>

        {/* 24h 涨跌 */}
        <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
          <p class="text-xs text-gray-500 mb-1">24h 涨跌</p>
          <Show
            when={!tickerLoading() && !tickerError()}
            fallback={
              <p class="text-xl font-bold text-gray-400">
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
              <p class="text-xl font-bold text-green-500">
                {ticker()
                  ? `+${ticker()!.price_change_pct.toFixed(2)}% / +${formatPrice(ticker()!.price_change)}`
                  : '--'}
              </p>
            </Show>
          </Show>
        </div>

        {/* 24h 最高 */}
        <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
          <p class="text-xs text-gray-500 mb-1">24h 最高</p>
          <Show
            when={!tickerLoading() && !tickerError()}
            fallback={
              <p class="text-xl font-bold text-gray-400">
                {tickerLoading() ? '--' : '数据不可用'}
              </p>
            }
          >
            <p class="text-xl font-bold text-gray-800">
              {ticker() ? formatPrice(ticker()!.high_24h) : '--'}
            </p>
          </Show>
        </div>

        {/* 24h 最低 */}
        <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
          <p class="text-xs text-gray-500 mb-1">24h 最低</p>
          <Show
            when={!tickerLoading() && !tickerError()}
            fallback={
              <p class="text-xl font-bold text-gray-400">
                {tickerLoading() ? '--' : '数据不可用'}
              </p>
            }
          >
            <p class="text-xl font-bold text-gray-800">
              {ticker() ? formatPrice(ticker()!.low_24h) : '--'}
            </p>
          </Show>
        </div>

        {/* 24h 成交量 */}
        <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
          <p class="text-xs text-gray-500 mb-1">24h 成交量</p>
          <Show
            when={!tickerLoading() && !tickerError()}
            fallback={
              <p class="text-xl font-bold text-gray-400">
                {tickerLoading() ? '--' : '数据不可用'}
              </p>
            }
          >
            <p class="text-xl font-bold text-gray-800">
              {ticker() ? formatVolume(ticker()!.volume_24h) : '--'}
            </p>
          </Show>
        </div>
      </div>

      {/* K线图 */}
      <div class="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
        <div class="px-4 py-3 border-b border-gray-200">
          <h3 class="text-sm font-semibold text-gray-700">K线图</h3>
        </div>
        <div class="p-4">
          <Show
            when={klinesLoading()}
            fallback={
              <Show
                when={klinesError()}
                fallback={
                  <Show
                    when={klines().length > 0}
                    fallback={
                      <div class="text-center py-12">
                        <p class="text-gray-400">暂无K线数据</p>
                      </div>
                    }
                  >
                    <canvas
                      ref={chartCanvas}
                      class="w-full"
                      style={{ height: '400px' }}
                    />
                  </Show>
                }
              >
                <div class="text-center py-12">
                  <p class="text-red-400">加载K线数据失败</p>
                </div>
              </Show>
            }
          >
            <div class="text-center py-12">
              <p class="text-gray-400">加载K线数据...</p>
            </div>
          </Show>
        </div>
      </div>

      {/* 订单簿 */}
      <div class="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
        <div class="px-4 py-3 border-b border-gray-200">
          <h3 class="text-sm font-semibold text-gray-700">订单簿</h3>
        </div>
        <div class="p-4">
          <Show
            when={orderbookLoading()}
            fallback={
              <Show
                when={orderbookError()}
                fallback={
                  <Show
                    when={orderbook() !== null}
                    fallback={
                      <div class="text-center py-12">
                        <p class="text-gray-400">暂无订单簿数据</p>
                      </div>
                    }
                  >
                    <div class="grid grid-cols-2 gap-6">
                      {/* 买盘 (bids) */}
                      <div>
                        <div class="grid grid-cols-3 text-xs font-medium text-gray-500 mb-2 px-2">
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
                                  class="absolute inset-0 bg-green-50 rounded"
                                  style={{ width: `${pct}%` }}
                                />
                                <span class="relative text-green-600 font-mono">
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
                        <div class="grid grid-cols-3 text-xs font-medium text-gray-500 mb-2 px-2">
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
                                  class="absolute inset-y-0 right-0 bg-red-50 rounded"
                                  style={{ width: `${pct}%` }}
                                />
                                <span class="relative text-red-600 font-mono">
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
                <div class="text-center py-12">
                  <p class="text-red-400">加载订单簿失败</p>
                </div>
              </Show>
            }
          >
            <div class="text-center py-12">
              <p class="text-gray-400">加载订单簿...</p>
            </div>
          </Show>
        </div>
      </div>

      {/* 余额表格 (可折叠) */}
      <div class="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
        <button
          class="w-full px-4 py-3 border-b border-gray-200 flex items-center justify-between hover:bg-gray-50 transition-colors"
          onClick={() => setBalancesExpanded(!balancesExpanded())}
        >
          <div class="flex items-center gap-2">
            <h3 class="text-sm font-semibold text-gray-700">账户余额</h3>
            <Show when={balancesLoading()}>
              <span class="text-xs text-gray-400">加载中...</span>
            </Show>
            <Show when={balancesError()}>
              <span class="text-xs text-red-400">加载失败</span>
            </Show>
          </div>
          <svg
            class={`w-4 h-4 text-gray-400 transition-transform ${balancesExpanded() ? 'rotate-180' : ''}`}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width="2"
          >
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
          </svg>
        </button>

        <Show when={balancesExpanded()}>
          <div class="p-4">
            <Show
              when={!balancesError()}
              fallback={
                <div class="text-center py-8">
                  <p class="text-red-400">加载余额失败，请重试</p>
                </div>
              }
            >
              <Show
                when={balances().length > 0}
                fallback={
                  <div class="text-center py-8">
                    <p class="text-gray-400">暂无余额数据</p>
                  </div>
                }
              >
                <div class="overflow-x-auto">
                  <table class="w-full text-sm">
                    <thead>
                      <tr class="border-b border-gray-200 bg-gray-50">
                        <th class="text-left px-4 py-2 font-medium text-gray-600">币种</th>
                        <th class="text-right px-4 py-2 font-medium text-gray-600">可用余额</th>
                        <th class="text-right px-4 py-2 font-medium text-gray-600">冻结余额</th>
                        <th class="text-right px-4 py-2 font-medium text-gray-600">总余额</th>
                      </tr>
                    </thead>
                    <tbody>
                      <For each={balances()}>
                        {(b) => (
                          <tr class="border-b border-gray-100 hover:bg-gray-50 transition-colors">
                            <td class="px-4 py-2 font-medium text-gray-800">{b.currency}</td>
                            <td class="px-4 py-2 text-right text-gray-600 font-mono">
                              {b.available.toFixed(8)}
                            </td>
                            <td class="px-4 py-2 text-right text-gray-600 font-mono">
                              {b.frozen.toFixed(8)}
                            </td>
                            <td class="px-4 py-2 text-right text-gray-800 font-mono font-medium">
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
        </Show>
      </div>
    </div>
  )
}

export default Market
