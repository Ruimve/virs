import { type Component, createSignal, createEffect, createMemo, Show, For, onMount } from 'solid-js'
import { api } from '../lib/api'
import KlineChart from '../components/KlineChart'
import { type OverlayLine, computeSMA, computeEMA, computeBBands } from '../utils/indicators'
import { useMarket } from '../lib/market-context'
import { useKlineWs, type KlineWsEvent } from '../lib/ws'

// ── 类型 ──

interface TickerData { [key: string]: unknown }
interface KlineItem { open_time: number; open: number; high: number; low: number; close: number; volume: number; [key: string]: unknown }
interface OrderbookEntry { price: number; quantity: number }
interface OrderbookData { bids: OrderbookEntry[]; asks: OrderbookEntry[] }
interface BalanceItem { currency: string; available: number; frozen: number; total: number }

// ── 工具 ──

function fmtP(v: number | undefined | null, d = 2): string {
  if (v == null || isNaN(v as number)) return '--'
  return Number(v).toFixed(d)
}
function fmtVol(v: number | undefined | null): string {
  if (v == null || isNaN(v as number)) return '--'
  const n = Number(v)
  if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B'
  if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M'
  if (n >= 1e3) return (n / 1e3).toFixed(2) + 'K'
  return n.toFixed(2)
}
function fmtPct(v: number | undefined | null): string {
  if (v == null || isNaN(v as number)) return '--'
  const n = Number(v)
  return `${n >= 0 ? '+' : ''}${n.toFixed(2)}%`
}

function cumEntries(entries: OrderbookEntry[]): { e: OrderbookEntry; cum: number }[] {
  let c = 0
  return entries.map(e => { c += e.quantity; return { e, cum: c } })
}

// ── 组件 ──

const Market: Component = () => {
  const market = useMarket()
  const [exchange, setExchange] = createSignal('binance')
  const [symbol, setSymbol] = createSignal('BTC/USDT')
  const [interval, setInterval_] = createSignal('1h')
  const [tab, setTab] = createSignal<'overview' | 'balance'>('overview')

  const [ticker, setTicker] = createSignal<TickerData | null>(null)
  const [klines, setKlines] = createSignal<KlineItem[]>([])
  const [orderbook, setOrderbook] = createSignal<OrderbookData | null>(null)
  const [balances, setBalances] = createSignal<BalanceItem[]>([])

  const [loading, setLoading] = createSignal(false)
  const [error, setError] = createSignal('')
  const [balLoading, setBalLoading] = createSignal(false)

  const [selectedIndicators, setSelectedIndicators] = createSignal<string[]>(['sma20'])
  const toggleInd = (name: string) => setSelectedIndicators(p => p.includes(name) ? p.filter(i => i !== name) : [...p, name])

  // 请求版本号：每次切换周期/交易所/市场类型时递增，用于丢弃过期的异步响应
  let fetchVersion = 0
  // 记录当前已加载数据对应的周期，WS 推送据此过滤
  let loadedTimeframe = ''

  async function ensureKlineSubscribed() {
    try {
      const res = await api.post('/kline/subscribe', {
        exchange: exchange(),
        symbol: symbol(),
        market_type: market.marketType(),
        timeframe: interval(),
      })
      return res.success
    } catch {
      return false
    }
  }

  async function fetchKlinesFromEngine(version: number): Promise<boolean> {
    try {
      const params = new URLSearchParams({
        exchange: exchange(),
        symbol: symbol(),
        timeframe: interval(),
      })
      const res = await api.get<any>(`/kline/data?${params}`)
      // 版本号已过期，丢弃响应
      if (version !== fetchVersion) return false
      if (res.success && res.data) {
        const d = res.data
        const candles = Array.isArray(d) ? d : (d.SingleTimeframe || d.AllTimeframes || [])
        if (Array.isArray(candles) && candles.length > 0) {
          setKlines(candles)
          loadedTimeframe = interval()
          return true
        }
      }
      return false
    } catch {
      return false
    }
  }

  async function fetchKlinesFallback(version: number) {
    try {
      const params = { exchange: exchange(), symbol: symbol(), market_type: market.marketType() }
      const k = await api.get<any>(`/market/klines?${new URLSearchParams({ ...params, interval: interval(), limit: '500' } as any)}`)
      if (version !== fetchVersion) return
      if (k.success && k.data) {
        const candles = Array.isArray(k.data) ? k.data : (k.data.candles || [])
        if (candles.length > 0) {
          setKlines(candles)
          loadedTimeframe = interval()
        }
      }
    } catch { /* */ }
  }

  async function fetchAll() {
    const version = ++fetchVersion
    setLoading(true); setError('')
    // 立即清空旧数据，阻止 WS 推送追加到旧数据上
    setKlines([])
    loadedTimeframe = ''
    try {
      const params = { exchange: exchange(), symbol: symbol(), market_type: market.marketType() }
      const [t, o] = await Promise.all([
        api.get<TickerData>(`/market/ticker?${new URLSearchParams(params as any)}`),
        api.get<OrderbookData>(`/market/orderbook?${new URLSearchParams({ ...params, depth: '20' } as any)}`),
      ])
      if (version !== fetchVersion) return
      if (t.success) setTicker(t.data ?? null)
      if (o.success) setOrderbook(o.data ?? null)

      const subscribed = await ensureKlineSubscribed()
      if (version !== fetchVersion) return
      if (subscribed) {
        const loaded = await fetchKlinesFromEngine(version)
        if (version !== fetchVersion) return
        if (!loaded) {
          await fetchKlinesFallback(version)
        }
      } else {
        await fetchKlinesFallback(version)
      }
    } catch (e: any) { if (version === fetchVersion) setError(e.message || '加载失败') }
    finally { if (version === fetchVersion) setLoading(false) }
  }

  async function fetchBalances() {
    setBalLoading(true)
    try {
      const res = await api.get<BalanceItem[]>(`/market/balances?${new URLSearchParams({ exchange: exchange(), market_type: market.marketType() } as any)}`)
      if (res.success && res.data) setBalances(res.data.filter(b => b.available > 0 || b.frozen > 0 || b.total > 0))
    } catch { /* */ }
    finally { setBalLoading(false) }
  }

  // 自动加载
  onMount(fetchAll)
  createEffect(() => { interval(); fetchAll() })
  createEffect(() => { market.marketType(); fetchAll() })

  useKlineWs((event: KlineWsEvent) => {
    const curExchange = exchange()
    const curSymbol = symbol()
    const curInterval = interval()

    if (event.exchange.toLowerCase() !== curExchange.toLowerCase()) return
    if (event.symbol.toUpperCase() !== curSymbol.toUpperCase()) return
    if (event.timeframe !== curInterval) return
    if (!loadedTimeframe) return

    const candle = event.candle
    setKlines(prev => {
      const idx = prev.findIndex(k => k.open_time === candle.open_time)
      if (idx !== -1) {
        const next = [...prev]
        next[idx] = { ...next[idx], open: candle.open, high: candle.high, low: candle.low, close: candle.close, volume: candle.volume }
        return next
      }
      return [...prev, {
        open_time: candle.open_time,
        open: candle.open,
        high: candle.high,
        low: candle.low,
        close: candle.close,
        volume: candle.volume,
      }]
    })
  }, () => {
    fetchAll()
  })

  // ── 指标 ──

  const overlays = createMemo(() => {
    const data = klines()
    if (!data || data.length < 2) return []
    const closes = data.map(k => k.close)
    const result: OverlayLine[] = []
    for (const ind of selectedIndicators()) {
      if (ind === 'sma20') {
        const v = computeSMA(closes, 20)
        result.push({ name: 'SMA 20', data: v.map((val, i) => ({ time: Math.floor(data[i].open_time / 1000), value: val })).filter(d => d.value !== null) as any, color: '#f59e0b' })
      } else if (ind === 'ema12') {
        const v = computeEMA(closes, 12)
        result.push({ name: 'EMA 12', data: v.map((val, i) => ({ time: Math.floor(data[i].open_time / 1000), value: val })).filter(d => d.value !== null) as any, color: '#6366f1' })
      } else if (ind === 'bbands') {
        const bb = computeBBands(closes, 20, 2)
        result.push({ name: 'BB Upper', data: bb.upper.map((val, i) => ({ time: Math.floor(data[i].open_time / 1000), value: val })).filter(d => d.value !== null) as any, color: 'rgba(239,68,68,0.4)', lineWidth: 1 })
        result.push({ name: 'BB Mid', data: bb.middle.map((val, i) => ({ time: Math.floor(data[i].open_time / 1000), value: val })).filter(d => d.value !== null) as any, color: 'rgba(156,163,175,0.4)', lineWidth: 1 })
        result.push({ name: 'BB Lower', data: bb.lower.map((val, i) => ({ time: Math.floor(data[i].open_time / 1000), value: val })).filter(d => d.value !== null) as any, color: 'rgba(16,185,129,0.4)', lineWidth: 1 })
      }
    }
    return result
  })

  // Ticker 字段兼容
  const lastPrice = () => (ticker()?.last ?? ticker()?.last_price ?? 0) as number
  const priceChange = () => (ticker()?.price_change_24h ?? ticker()?.price_change ?? 0) as number
  const priceChangePct = () => (ticker()?.price_change_pct_24h ?? ticker()?.price_change_pct ?? 0) as number
  const high24h = () => (ticker()?.high_24h ?? 0) as number
  const low24h = () => (ticker()?.low_24h ?? 0) as number
  const vol24h = () => (ticker()?.volume_24h ?? 0) as number

  // 订单簿兼容（元组 or 对象）
  const bids = () => {
    const raw = orderbook()?.bids
    if (!raw || !Array.isArray(raw)) return []
    return raw.map((b: any) => typeof b === 'object' && !Array.isArray(b) ? b : { price: b[0], quantity: b[1] })
  }
  const asks = () => {
    const raw = orderbook()?.asks
    if (!raw || !Array.isArray(raw)) return []
    return raw.map((a: any) => typeof a === 'object' && !Array.isArray(a) ? a : { price: a[0], quantity: a[1] })
  }

  // 余额兼容
  const balancesCompat = () => balances().map((b: any) => ({
    currency: b.currency ?? b.asset ?? '',
    available: b.available ?? b.free ?? 0,
    frozen: b.frozen ?? b.used ?? 0,
    total: b.total ?? 0,
  }))

  // ── 样式 ──

  const inp = "w-full bg-slate-50 border border-slate-200 rounded-lg px-3 py-[7px] text-[13px] text-slate-800 placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-300 focus:bg-white transition-all"
  const sel = "w-full bg-slate-50 border border-slate-200 rounded-lg px-3 py-[7px] text-[13px] text-slate-800 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-300 focus:bg-white transition-all appearance-none cursor-pointer"

  return (
    <div class="space-y-4">
      {/* ═══ 顶部工具栏 ═══ */}
      <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
        <div class="px-5 py-3.5">
          <div class="flex flex-wrap items-center gap-3">
            <div class="w-[105px]">
              <select class={sel} value={exchange()} onChange={e => setExchange(e.currentTarget.value)}>
                <option value="binance">Binance</option><option value="okx">OKX</option><option value="bybit">Bybit</option>
              </select>
            </div>
            <div class="w-[130px]">
              <input type="text" class={inp} value={symbol()} onInput={e => setSymbol(e.currentTarget.value)} placeholder="BTC/USDT" />
            </div>
            <div class="flex bg-slate-100 rounded-lg p-[3px] gap-[2px]">
              {['1m', '5m', '15m', '1h', '4h', '1d'].map(tf => (
                <button type="button" class={`px-2.5 py-[4px] text-[12px] rounded-md font-medium transition-all duration-200 ${interval() === tf ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-500 hover:text-slate-700'}`}
                  onClick={() => setInterval_(tf)}>{tf}</button>
              ))}
            </div>
            <button class="px-4 py-[7px] bg-indigo-600 text-white text-[13px] font-semibold rounded-lg hover:bg-indigo-700 transition-all disabled:opacity-50 shadow-sm shadow-indigo-200"
              disabled={loading()} onClick={fetchAll}>
              {loading() ? '加载中...' : '刷新'}
            </button>

            {/* Tab 切换 */}
            <div class="ml-auto flex bg-slate-100 rounded-lg p-[3px] gap-[2px]">
              <button type="button" class={`px-3.5 py-[4px] text-[12px] rounded-md font-medium transition-all ${tab() === 'overview' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-500 hover:text-slate-700'}`}
                onClick={() => setTab('overview')}>行情</button>
              <button type="button" class={`px-3.5 py-[4px] text-[12px] rounded-md font-medium transition-all ${tab() === 'balance' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-500 hover:text-slate-700'}`}
                onClick={() => { setTab('balance'); fetchBalances() }}>余额</button>
            </div>
          </div>
        </div>

        {/* Ticker 横条 */}
        <Show when={tab() === 'overview' && ticker()}>
          <div class="px-5 py-3 border-t border-slate-100 bg-slate-50/50">
            <div class="flex flex-wrap items-center gap-x-6 gap-y-1.5">
              {/* 最新价 — 突出显示 */}
              <div class="flex items-baseline gap-2">
                <span class="text-[11px] font-medium text-slate-400 uppercase tracking-wider">最新价</span>
                <span class="text-xl font-bold text-slate-900 font-mono tracking-tight">{fmtP(lastPrice())}</span>
              </div>
              {/* 涨跌幅 */}
              <div class={`flex items-center gap-1.5 px-2.5 py-1 rounded-md ${priceChangePct() >= 0 ? 'bg-emerald-50' : 'bg-red-50'}`}>
                <span class={`text-[14px] font-bold font-mono ${priceChangePct() >= 0 ? 'text-emerald-600' : 'text-red-500'}`}>{fmtPct(priceChangePct())}</span>
                <span class={`text-[12px] font-mono ${priceChangePct() >= 0 ? 'text-emerald-500' : 'text-red-400'}`}>
                  {priceChangePct() >= 0 ? '+' : ''}{fmtP(priceChange())}
                </span>
              </div>
              {/* 24h 高低 */}
              <div class="flex items-center gap-4 text-[12px]">
                <span class="text-slate-400">高 <b class="text-slate-700 font-mono">{fmtP(high24h())}</b></span>
                <span class="text-slate-400">低 <b class="text-slate-700 font-mono">{fmtP(low24h())}</b></span>
              </div>
              {/* 24h 量 */}
              <span class="text-[12px] text-slate-400">量 <b class="text-slate-700 font-mono">{fmtVol(vol24h())}</b></span>
            </div>
          </div>
        </Show>
      </div>

      {/* ═══ 行情总览 ═══ */}
      <Show when={tab() === 'overview'}>
        <Show when={error()}>
          <div class="p-3 bg-red-50 border border-red-200/60 rounded-xl text-[13px] text-red-600 flex items-center gap-2">
            <svg class="w-4 h-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" /></svg>
            {error()}
          </div>
        </Show>

        {/* K 线 + 订单簿 并排 */}
        <div class="grid grid-cols-1 lg:grid-cols-[1fr_320px] gap-4">
          {/* K 线图 */}
          <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
            <div class="px-4 py-3 border-b border-slate-100 flex items-center justify-between">
              <div class="flex items-center gap-2">
                <span class="text-[12px] font-semibold text-slate-500 uppercase tracking-wider">K 线</span>
                <span class="text-[11px] text-slate-400">{symbol()} · {interval()}</span>
              </div>
              <div class="flex items-center gap-1.5">
                {(['sma20', 'ema12', 'bbands'] as const).map(ind => (
                  <button type="button" class={`px-2 py-[3px] rounded-md text-[11px] font-medium transition-colors ${selectedIndicators().includes(ind) ? 'bg-indigo-100 text-indigo-700' : 'bg-slate-100 text-slate-500 hover:bg-slate-200'}`}
                    onClick={() => toggleInd(ind)}>
                    {ind === 'sma20' ? 'SMA 20' : ind === 'ema12' ? 'EMA 12' : 'BBands'}
                  </button>
                ))}
              </div>
            </div>
            <div class="p-3">
              <Show when={!loading() || klines().length > 0} fallback={
                <div class="flex items-center justify-center h-[420px]"><div class="w-5 h-5 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" /></div>
              }>
                <Show when={klines().length > 0} fallback={
                  <div class="flex items-center justify-center h-[420px] text-slate-400 text-[13px]">点击「刷新」加载 K 线数据</div>
                }>
                  <KlineChart
                    data={klines().map(k => ({ time: Math.floor(k.open_time / 1000), open: k.open, high: k.high, low: k.low, close: k.close, volume: k.volume }))}
                    height={420}
                    overlays={overlays()}
                  />
                </Show>
              </Show>
            </div>
          </div>

          {/* 订单簿 */}
          <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
            <div class="px-4 py-3 border-b border-slate-100">
              <span class="text-[12px] font-semibold text-slate-500 uppercase tracking-wider">订单簿</span>
            </div>
            <div class="p-3">
              <Show when={!loading() || orderbook() !== null} fallback={
                <div class="flex items-center justify-center h-[420px]"><div class="w-5 h-5 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" /></div>
              }>
                <Show when={orderbook() !== null} fallback={
                  <div class="flex items-center justify-center h-[420px] text-slate-400 text-[13px]">暂无数据</div>
                }>
                  <div class="space-y-0">
                    {/* 表头 */}
                    <div class="grid grid-cols-3 text-[10px] font-semibold text-slate-400 uppercase tracking-wider px-2 pb-1.5">
                      <span>价格</span><span class="text-right">数量</span><span class="text-right">累计</span>
                    </div>

                    {/* 卖盘（倒序，最高价在上） */}
                    <div class="space-y-px">
                      <For each={[...cumEntries(asks())].reverse()}>
                        {({ e, cum }) => {
                          const maxCum = cumEntries(asks()).slice(-1)[0]?.cum || 1
                          return (
                            <div class="relative grid grid-cols-3 text-[12px] py-[3px] px-2 rounded-sm hover:bg-red-50/30 transition-colors">
                              <div class="absolute inset-y-0 right-0 bg-red-50/50 rounded-sm" style={{ width: `${(cum / maxCum) * 100}%` }} />
                              <span class="relative text-red-500 font-mono">{fmtP(e.price)}</span>
                              <span class="relative text-right text-slate-500 font-mono">{e.quantity.toFixed(4)}</span>
                              <span class="relative text-right text-slate-400 font-mono text-[11px]">{cum.toFixed(2)}</span>
                            </div>
                          )
                        }}
                      </For>
                    </div>

                    {/* 中间价差 */}
                    <Show when={lastPrice() > 0}>
                      <div class="py-2 px-2 text-center border-y border-slate-100 my-1">
                        <span class={`text-[15px] font-bold font-mono ${priceChangePct() >= 0 ? 'text-emerald-600' : 'text-red-500'}`}>
                          {fmtP(lastPrice())}
                        </span>
                        <span class={`text-[11px] font-mono ml-1.5 ${priceChangePct() >= 0 ? 'text-emerald-500' : 'text-red-400'}`}>
                          {fmtPct(priceChangePct())}
                        </span>
                      </div>
                    </Show>

                    {/* 买盘 */}
                    <div class="space-y-px">
                      <For each={cumEntries(bids())}>
                        {({ e, cum }) => {
                          const maxCum = cumEntries(bids()).slice(-1)[0]?.cum || 1
                          return (
                            <div class="relative grid grid-cols-3 text-[12px] py-[3px] px-2 rounded-sm hover:bg-emerald-50/30 transition-colors">
                              <div class="absolute inset-y-0 right-0 bg-emerald-50/50 rounded-sm" style={{ width: `${(cum / maxCum) * 100}%` }} />
                              <span class="relative text-emerald-600 font-mono">{fmtP(e.price)}</span>
                              <span class="relative text-right text-slate-500 font-mono">{e.quantity.toFixed(4)}</span>
                              <span class="relative text-right text-slate-400 font-mono text-[11px]">{cum.toFixed(2)}</span>
                            </div>
                          )
                        }}
                      </For>
                    </div>
                  </div>
                </Show>
              </Show>
            </div>
          </div>
        </div>
      </Show>

      {/* ═══ 余额 ═══ */}
      <Show when={tab() === 'balance'}>
        <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
          <div class="px-5 py-3 border-b border-slate-100 flex items-center justify-between">
            <span class="text-[12px] font-semibold text-slate-500 uppercase tracking-wider">账户余额</span>
            <span class="text-[11px] text-slate-400">{exchange()}</span>
          </div>
          <Show when={!balLoading()} fallback={<div class="flex items-center justify-center py-16"><div class="w-5 h-5 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" /></div>}>
            <Show when={balancesCompat().length > 0} fallback={<div class="text-center py-16 text-slate-400 text-[13px]">暂无余额数据</div>}>
              <div class="overflow-x-auto">
                <table class="w-full text-[13px]">
                  <thead>
                    <tr class="border-b border-slate-100 bg-slate-50/50">
                      <th class="text-left py-2.5 px-5 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">币种</th>
                      <th class="text-right py-2.5 px-5 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">可用</th>
                      <th class="text-right py-2.5 px-5 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">冻结</th>
                      <th class="text-right py-2.5 px-5 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">总计</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={balancesCompat()}>
                      {(b, i) => (
                        <tr class={`border-b border-slate-50 hover:bg-slate-50/50 transition-colors ${i() % 2 ? 'bg-slate-50/20' : ''}`}>
                          <td class="py-2.5 px-5 font-semibold text-slate-800">{b.currency}</td>
                          <td class="py-2.5 px-5 text-right text-slate-500 font-mono">{b.available.toFixed(8)}</td>
                          <td class="py-2.5 px-5 text-right text-slate-500 font-mono">{b.frozen.toFixed(8)}</td>
                          <td class="py-2.5 px-5 text-right text-slate-800 font-mono font-semibold">{b.total.toFixed(8)}</td>
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
  )
}

export default Market
