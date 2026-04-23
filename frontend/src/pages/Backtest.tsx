import { type Component, createSignal, createMemo, Show, For, onMount } from 'solid-js'
import { api, fetchPlugins, validateScript, type PaginatedResponse, type Plugin } from '../lib/api'
import KlineChart from '../components/KlineChart'
import EquityChart from '../components/EquityChart'
import { useMarket } from '../lib/market-context'
import { type OverlayLine, computeSMA, computeEMA } from '../utils/indicators'

// ---- 类型定义 ----

interface BacktestRequest {
  strategy_type: string
  symbol: string
  exchange: string
  timeframe: string
  start_date: string | null
  end_date: string | null
  initial_balance: number
  indicator_config: Record<string, unknown>
  trading_config: Record<string, unknown>
}

interface BacktestTrade {
  entry_time: string
  exit_time: string
  side: 'long' | 'short'
  entry_price: number
  exit_price: number
  quantity: number
  pnl: number
  pnl_pct: number
  commission: number
  funding_fee?: number
}

interface FundingEvent {
  time: string
  rate: number
  amount: number
  side: 'long' | 'short'
}

interface BacktestResult {
  id: string
  strategy_name: string
  symbol: string
  exchange: string
  initial_balance: number
  final_balance: number
  total_return_pct: number
  max_drawdown_pct: number
  sharpe_ratio: number
  sortino_ratio: number
  win_rate: number
  total_trades: number
  profit_trades: number
  loss_trades: number
  avg_profit: number
  avg_loss: number
  profit_factor: number
  max_consecutive_wins: number
  max_consecutive_losses: number
  trades: BacktestTrade[]
  equity_curve: [string, number][]
  funding_events?: FundingEvent[]
  klines?: { time: number; open: number; high: number; low: number; close: number; volume: number }[]
}

interface BacktestSummary {
  id: string
  strategy_name: string
  symbol: string
  exchange: string
  initial_balance: number
  final_balance: number
  total_return_pct: number
  sharpe_ratio: number
  max_drawdown_pct: number
}

// ---- 默认配置 ----

const DEFAULT_TRADING_CONFIG: Record<string, unknown> = {
  stop_loss_pct: 0.03,
  take_profit_pct: 0.06,
  commission_rate: 0.001,
  slippage: 0.0005,
  position_pct: 1.0,
  leverage: 1,
  trailing_stop_pct: null,
  trailing_activation_pct: null,
  trade_direction: 'long',
}

const DEFAULT_LUA_SCRIPT = `-- VIRS Lua Strategy: EMA Crossover with RSI Filter
-- Available functions: sma(period), ema(period), rsi(period)
-- Available data: klines (table), current_idx (number), params (table)
-- Return: 1 = buy, -1 = sell, 0 = hold

function signal()
  local fast = ema(params.fast_period or 12)
  local slow = ema(params.slow_period or 26)
  local rsi_val = rsi(params.rsi_period or 14)

  if fast > slow and rsi_val > (params.rsi_floor or 45) then
    return 1
  elseif fast < slow then
    return -1
  end
  return 0
end`

// ---- 工具函数 ----

function fmt(n: number | undefined | null, d = 2): string {
  if (n == null || isNaN(n)) return '-'
  return n.toFixed(d)
}

function fmtPct(n: number | undefined | null): string {
  if (n == null || isNaN(n)) return '-'
  return `${n >= 0 ? '+' : ''}${fmt(n)}%`
}

function fmtDT(s: string): string {
  if (!s) return '-'
  try {
    return new Date(s).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
  } catch { return s }
}

// ---- 折叠箭头组件 ----

function Chevron(props: { open: boolean }) {
  return (
    <svg class={`w-3.5 h-3.5 transition-transform duration-200 ${props.open ? 'rotate-90' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
      <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
    </svg>
  )
}

// ---- 组件 ----

const Backtest: Component = () => {
  // 交易配置辅助
  const [tc, setTc] = createSignal(JSON.stringify(DEFAULT_TRADING_CONFIG))
  const getTC = () => { try { return JSON.parse(tc()) } catch { return { ...DEFAULT_TRADING_CONFIG } } }
  const setTC = (f: string, v: unknown) => setTc(JSON.stringify({ ...getTC(), [f]: v }))

  // 插件
  const [plugins, setPlugins] = createSignal<Plugin[]>([])
  const [pluginsLoading, setPluginsLoading] = createSignal(false)
  const [pluginsError, setPluginsError] = createSignal('')

  // 表单
  const [mode, setMode] = createSignal<'plugin' | 'script'>('plugin')
  const [strategyType, setStrategyType] = createSignal('custom')
  const [symbol, setSymbol] = createSignal('BTCUSDT')
  const [exchange, setExchange] = createSignal('binance')
  const market = useMarket()
  const [timeframe, setTimeframe] = createSignal('1h')
  const [startDate, setStartDate] = createSignal((() => { const d = new Date(); d.setFullYear(d.getFullYear() - 1); return d.toISOString().split('T')[0] })())
  const [endDate, setEndDate] = createSignal(new Date().toISOString().split('T')[0])
  const [initialBalance, setInitialBalance] = createSignal(10000)
  const [indicatorConfig, setIndicatorConfig] = createSignal('{}')
  const [strategyCode, setStrategyCode] = createSignal(DEFAULT_LUA_SCRIPT)
  const [scriptParams, setScriptParams] = createSignal('{"fast_period": 12, "slow_period": 26, "rsi_period": 14, "rsi_floor": 45}')

  // 已保存策略
  const [savedStrategies, setSavedStrategies] = createSignal<Array<{ id: string; name: string; strategy_code?: string; indicator_config?: Record<string, unknown> }>>([])

  // 折叠
  const [showStrategy, setShowStrategy] = createSignal(false)
  const [showAdvanced, setShowAdvanced] = createSignal(false)

  // 运行
  const [running, setRunning] = createSignal(false)
  const [runError, setRunError] = createSignal('')

  // 脚本验证
  const [scriptValidating, setScriptValidating] = createSignal(false)
  const [scriptValidationResult, setScriptValidationResult] = createSignal<{ valid: boolean; error?: string } | null>(null)

  // 结果
  const [result, setResult] = createSignal<BacktestResult | null>(null)

  // AI
  const [aiLoading, setAiLoading] = createSignal(false)
  const [aiSuggestion, setAiSuggestion] = createSignal<string | null>(null)

  // 历史
  const [historyItems, setHistoryItems] = createSignal<BacktestSummary[]>([])
  const [historyPage, setHistoryPage] = createSignal(1)
  const [historyTotalPages, setHistoryTotalPages] = createSignal(0)
  const [historyLoading, setHistoryLoading] = createSignal(false)
  const [historyError, setHistoryError] = createSignal('')

  // ── 数据加载 ──

  async function loadPlugins() {
    setPluginsLoading(true); setPluginsError('')
    try {
      const res = await fetchPlugins()
      if (res.success && res.data) {
        setPlugins(res.data)
        if (res.data.length > 0) {
          const first = res.data[0]
          setStrategyType(first.name)
          const config: Record<string, unknown> = { plugin: first.name }
          for (const p of first.params) config[p.name] = p.default
          setIndicatorConfig(JSON.stringify(config, null, 2))
        }
      } else setPluginsError(res.error || '加载失败')
    } catch (e) { setPluginsError(e instanceof Error ? e.message : '加载失败') }
    finally { setPluginsLoading(false) }
  }

  function handleStrategyTypeChange(name: string) {
    setStrategyType(name)
    const plugin = plugins().find(p => p.name === name)
    if (plugin) {
      const config: Record<string, unknown> = { plugin: plugin.name }
      for (const p of plugin.params) config[p.name] = p.default
      setIndicatorConfig(JSON.stringify(config, null, 2))
    }
  }

  async function loadSavedStrategies() {
    try {
      const res = await api.get<PaginatedResponse<any>>('/strategies?page=1&page_size=50')
      if (res.success && res.data?.items) setSavedStrategies(res.data.items.filter((s: any) => s.strategy_mode === 'script' && s.strategy_code))
    } catch { /* */ }
  }

  function handleSelectStrategy(id: string) {
    const s = savedStrategies().find(st => st.id === id)
    if (!s) return
    if (s.strategy_code) { setStrategyCode(s.strategy_code); setScriptValidationResult(null) }
    if (s.indicator_config) {
      const { plugin, strategy_code, ...params } = s.indicator_config
      if (Object.keys(params).length > 0) setScriptParams(JSON.stringify(params, null, 2))
    }
  }

  async function handleValidateScript() {
    setScriptValidating(true); setScriptValidationResult(null)
    try {
      const res = await validateScript(strategyCode())
      if (res.success && res.data) setScriptValidationResult(res.data)
      else setScriptValidationResult({ valid: false, error: res.error || '验证失败' })
    } catch (e) { setScriptValidationResult({ valid: false, error: e instanceof Error ? e.message : '网络错误' }) }
    finally { setScriptValidating(false) }
  }

  async function loadHistory(page: number) {
    setHistoryLoading(true); setHistoryError('')
    try {
      const res = await api.get<PaginatedResponse<BacktestSummary>>(`/backtest/list?page=${page}&page_size=10`)
      if (res.success && res.data) { setHistoryItems(res.data.items); setHistoryTotalPages(res.data.total_pages); setHistoryPage(res.data.page) }
      else setHistoryError(res.error || '加载失败')
    } catch (e) { setHistoryError(e instanceof Error ? e.message : '加载失败') }
    finally { setHistoryLoading(false) }
  }

  onMount(() => { loadPlugins(); loadSavedStrategies(); loadHistory(1) })

  // ── 运行回测 ──

  async function handleRun() {
    setRunning(true); setRunError(''); setResult(null)
    let parsedIndicator: Record<string, unknown>
    let parsedTrading: Record<string, unknown>
    try { parsedIndicator = JSON.parse(indicatorConfig()) } catch { setRunError('指标配置 JSON 格式错误'); setRunning(false); return }
    try { parsedTrading = JSON.parse(tc()) } catch { setRunError('交易配置 JSON 格式错误'); setRunning(false); return }
    if (mode() === 'script') {
      let pp: Record<string, unknown> = {}
      try { pp = JSON.parse(scriptParams()) } catch { setRunError('脚本参数 JSON 格式错误'); setRunning(false); return }
      parsedIndicator = { ...pp, ...parsedIndicator, strategy_code: strategyCode() }
    }
    parsedTrading.market_type = market.marketType()
    const req: BacktestRequest = {
      strategy_type: mode() === 'script' ? 'script' : strategyType(),
      symbol: symbol(), exchange: exchange(), timeframe: timeframe(),
      start_date: startDate() || null, end_date: endDate() || null,
      initial_balance: initialBalance(),
      indicator_config: parsedIndicator, trading_config: parsedTrading,
    }
    try {
      const res = await api.post<BacktestResult>('/backtest/run', req)
      if (res.success && res.data) setResult(res.data)
      else setRunError(res.error || '回测运行失败')
    } catch (e) { setRunError(e instanceof Error ? e.message : '回测运行失败') }
    finally { setRunning(false) }
  }

  async function handleViewDetail(id: string) {
    setResult(null); setRunError(''); setAiSuggestion(null)
    try {
      const res = await api.get<BacktestResult>(`/backtest/${id}`)
      if (res.success && res.data) { setResult(res.data); document.getElementById('backtest-result')?.scrollIntoView({ behavior: 'smooth', block: 'start' }) }
      else setRunError(res.error || '加载失败')
    } catch (e) { setRunError(e instanceof Error ? e.message : '加载失败') }
  }

  async function handleAiOptimize() {
    setAiLoading(true); setAiSuggestion(null)
    try {
      const r = result()!
      const res = await api.post<{ suggestion: string }>('/ai/optimize', {
        strategy_code: mode() === 'script' ? strategyCode() : undefined,
        backtest_summary: {
          total_return_pct: r.total_return_pct, sharpe_ratio: r.sharpe_ratio, sortino_ratio: r.sortino_ratio,
          win_rate: r.win_rate, profit_factor: r.profit_factor, max_drawdown_pct: r.max_drawdown_pct,
          total_trades: r.total_trades, profit_trades: r.profit_trades, loss_trades: r.loss_trades,
          avg_profit: r.avg_profit, avg_loss: r.avg_loss,
          max_consecutive_wins: r.max_consecutive_wins, max_consecutive_losses: r.max_consecutive_losses,
        },
      })
      if (res.success && res.data?.suggestion) setAiSuggestion(res.data.suggestion)
    } catch (e) { console.error('AI optimize failed:', e) }
    finally { setAiLoading(false) }
  }

  // ── 图表 ──

  const backtestKlineData = createMemo(() => {
    const r = result()
    if (!r) return []
    if (r.klines && r.klines.length > 0) return r.klines
    if (!r.trades || r.trades.length === 0) return []
    return r.trades.map(t => ({
      time: Math.floor(new Date(t.entry_time).getTime() / 1000),
      open: t.entry_price, high: Math.max(t.entry_price, t.exit_price),
      low: Math.min(t.entry_price, t.exit_price), close: t.exit_price,
    }))
  })

  const backtestMarkers = createMemo(() => {
    const r = result()
    if (!r || !r.trades) return []
    return r.trades.flatMap(t => {
      const long = t.side === 'long'
      return [
        { time: Math.floor(new Date(t.entry_time).getTime() / 1000), position: (long ? 'belowBar' : 'aboveBar') as 'belowBar' | 'aboveBar', color: long ? '#10b981' : '#ef4444', shape: (long ? 'arrowUp' : 'arrowDown') as 'arrowUp' | 'arrowDown', text: long ? 'Buy' : 'Sell' },
        { time: Math.floor(new Date(t.exit_time).getTime() / 1000), position: (long ? 'aboveBar' : 'belowBar') as 'belowBar' | 'aboveBar', color: long ? '#ef4444' : '#10b981', shape: (long ? 'arrowDown' : 'arrowUp') as 'arrowDown' | 'arrowUp', text: `Exit (${t.pnl >= 0 ? '+' : ''}${fmt(t.pnl_pct, 1)}%)` },
      ]
    })
  })

  const backtestOverlays = createMemo(() => {
    const data = backtestKlineData()
    if (!data || data.length < 2) return []
    const closes = data.map(k => k.close)
    const overlays: OverlayLine[] = []
    const sma20 = computeSMA(closes, 20)
    overlays.push({ name: 'SMA 20', data: sma20.map((v, i) => ({ time: data[i].time, value: v })).filter(d => d.value !== null) as any, color: '#f59e0b', lineWidth: 1 })
    const ema12 = computeEMA(closes, 12)
    overlays.push({ name: 'EMA 12', data: ema12.map((v, i) => ({ time: data[i].time, value: v })).filter(d => d.value !== null) as any, color: '#6366f1', lineWidth: 1 })
    return overlays
  })

  // ── 样式 ──

  const inp = "w-full bg-slate-50 border border-slate-200 rounded-lg px-3 py-[7px] text-[13px] text-slate-800 placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-300 focus:bg-white transition-all"
  const sel = "w-full bg-slate-50 border border-slate-200 rounded-lg px-3 py-[7px] text-[13px] text-slate-800 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-300 focus:bg-white transition-all appearance-none cursor-pointer"

  return (
    <div class="space-y-4">
      {/* ═══ 配置面板 ═══ */}
      <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
        {/* 主参数行 */}
        <div class="px-5 py-4">
          <div class="flex flex-wrap items-end gap-x-3 gap-y-2.5">
            {/* 模式切换 */}
            <div class="flex bg-slate-100 rounded-lg p-[3px] gap-[2px]">
              <button type="button" class={`px-3.5 py-[5px] text-[13px] rounded-md font-medium transition-all duration-200 ${mode() === 'plugin' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-500 hover:text-slate-700'}`} onClick={() => setMode('plugin')}>插件</button>
              <button type="button" class={`px-3.5 py-[5px] text-[13px] rounded-md font-medium transition-all duration-200 ${mode() === 'script' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-500 hover:text-slate-700'}`} onClick={() => { setMode('script'); setScriptValidationResult(null) }}>脚本</button>
            </div>

            <div class="w-[120px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px] tracking-wide uppercase">交易对</label>
              <input type="text" class={inp} value={symbol()} onInput={e => setSymbol(e.currentTarget.value)} placeholder="BTCUSDT" />
            </div>
            <div class="w-[105px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px] tracking-wide uppercase">交易所</label>
              <select class={sel} value={exchange()} onChange={e => setExchange(e.currentTarget.value)}>
                <option value="binance">Binance</option><option value="okx">OKX</option><option value="bybit">Bybit</option>
              </select>
            </div>
            <div class="w-[80px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px] tracking-wide uppercase">周期</label>
              <select class={sel} value={timeframe()} onChange={e => setTimeframe(e.currentTarget.value)}>
                <option value="1m">1m</option><option value="5m">5m</option><option value="15m">15m</option>
                <option value="1h">1h</option><option value="4h">4h</option><option value="1d">1d</option>
              </select>
            </div>
            <div class="w-[135px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px] tracking-wide uppercase">开始</label>
              <input type="date" class={inp} value={startDate()} onInput={e => setStartDate(e.currentTarget.value)} />
            </div>
            <div class="w-[135px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px] tracking-wide uppercase">结束</label>
              <input type="date" class={inp} value={endDate()} onInput={e => setEndDate(e.currentTarget.value)} />
            </div>
            <div class="w-[110px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px] tracking-wide uppercase">资金</label>
              <input type="number" class={inp} value={initialBalance()} onInput={e => setInitialBalance(Number(e.currentTarget.value))} min="0" step="1000" />
            </div>

            {/* 运行按钮 */}
            <div class="ml-auto">
              <button class="px-6 py-[7px] bg-indigo-600 text-white text-[13px] font-semibold rounded-lg hover:bg-indigo-700 active:bg-indigo-800 transition-all duration-150 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap shadow-sm shadow-indigo-200"
                disabled={running()} onClick={handleRun}>
                {running() ? (
                  <span class="flex items-center gap-2"><span class="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />运行中</span>
                ) : '运行回测'}
              </button>
            </div>
          </div>
        </div>

        {/* 分隔线 */}
        <div class="border-t border-slate-100" />

        {/* 交易参数行 */}
        <div class="px-5 py-3.5">
          <div class="flex flex-wrap items-end gap-x-3 gap-y-2.5">
            <div class="w-[72px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">止损 %</label>
              <input type="number" step="0.01" min="0" class={inp} value={String(getTC().stop_loss_pct ?? '')}
                onInput={e => { const v = parseFloat(e.currentTarget.value); setTC('stop_loss_pct', isNaN(v) ? null : v) }} />
            </div>
            <div class="w-[72px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">止盈 %</label>
              <input type="number" step="0.01" min="0" class={inp} value={String(getTC().take_profit_pct ?? '')}
                onInput={e => { const v = parseFloat(e.currentTarget.value); setTC('take_profit_pct', isNaN(v) ? null : v) }} />
            </div>
            <div class="w-[72px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">仓位</label>
              <input type="number" step="0.1" min="0.01" max="1" class={inp} value={String(getTC().position_pct ?? '')}
                onInput={e => { const v = parseFloat(e.currentTarget.value); setTC('position_pct', isNaN(v) ? 1.0 : Math.min(1, Math.max(0.01, v))) }} />
            </div>
            <div class="w-[72px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">杠杆</label>
              <input type="number" step="1" min="1" max="125" class={inp} value={String(getTC().leverage ?? 1)}
                onInput={e => { const v = parseInt(e.currentTarget.value) || 1; setTC('leverage', Math.min(125, Math.max(1, v))) }} />
            </div>
            <div class="w-[72px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">方向</label>
              <select class={sel} value={String(getTC().trade_direction ?? 'long')} onInput={e => setTC('trade_direction', e.currentTarget.value)}>
                <option value="long">做多</option><option value="short">做空</option><option value="both">双向</option>
              </select>
            </div>

            <button type="button" class="text-[12px] text-slate-400 hover:text-slate-600 transition-colors flex items-center gap-1 ml-1"
              onClick={() => setShowAdvanced(!showAdvanced())}>
              <Chevron open={showAdvanced()} />
              高级
            </button>
          </div>

          {/* 高级设置 */}
          <Show when={showAdvanced()}>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-x-3 gap-y-2.5 mt-3 pt-3 border-t border-slate-100">
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">手续费 %</label>
                <input type="number" step="0.001" min="0" class={inp} value={String(getTC().commission_rate ?? '')}
                  onInput={e => { const v = parseFloat(e.currentTarget.value); setTC('commission_rate', isNaN(v) ? null : v) }} />
              </div>
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">滑点 %</label>
                <input type="number" step="0.0001" min="0" class={inp} value={String(getTC().slippage ?? '')}
                  onInput={e => { const v = parseFloat(e.currentTarget.value); setTC('slippage', isNaN(v) ? null : v) }} />
              </div>
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">追踪止损 %</label>
                <input type="number" step="0.01" min="0" placeholder="关闭" class={inp}
                  onInput={e => { const v = e.currentTarget.value; setTC('trailing_stop_pct', v === '' ? null : parseFloat(v)) }} />
              </div>
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">追踪激活 %</label>
                <input type="number" step="0.01" min="0" placeholder="关闭" class={inp}
                  onInput={e => { const v = e.currentTarget.value; setTC('trailing_activation_pct', v === '' ? null : parseFloat(v)) }} />
              </div>
            </div>
          </Show>
        </div>

        {/* 策略配置（可折叠） */}
        <Show when={mode() === 'plugin'}>
          <div class="border-t border-slate-100">
            <button type="button" class="w-full px-5 py-2.5 text-[12px] text-slate-400 hover:text-slate-600 transition-colors flex items-center gap-1.5"
              onClick={() => setShowStrategy(!showStrategy())}>
              <Chevron open={showStrategy()} />
              策略配置
            </button>
            <Show when={showStrategy()}>
              <div class="px-5 pb-4 grid grid-cols-1 md:grid-cols-2 gap-3">
                <div>
                  <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">策略类型</label>
                  <Show when={!pluginsLoading()} fallback={<select class={sel} disabled><option>加载中...</option></select>}>
                    <Show when={pluginsError() === ''} fallback={<select class={sel} disabled><option>加载失败</option></select>}>
                      <select class={sel} value={strategyType()} onChange={e => handleStrategyTypeChange(e.currentTarget.value)}>
                        <For each={plugins()}>{(p) => <option value={p.name}>{p.name} - {p.description.slice(0, 30)}</option>}</For>
                      </select>
                    </Show>
                  </Show>
                </div>
                <div>
                  <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">指标参数 (JSON)</label>
                  <textarea class="w-full bg-slate-50 border border-slate-200 rounded-lg px-3 py-2 text-[13px] font-mono text-slate-800 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-300 focus:bg-white transition-all" rows={3}
                    value={indicatorConfig()} onInput={e => setIndicatorConfig(e.currentTarget.value)} />
                </div>
              </div>
            </Show>
          </div>
        </Show>

        {/* 脚本编辑器（可折叠） */}
        <Show when={mode() === 'script'}>
          <div class="border-t border-slate-100">
            <button type="button" class="w-full px-5 py-2.5 text-[12px] text-slate-400 hover:text-slate-600 transition-colors flex items-center gap-1.5"
              onClick={() => setShowStrategy(!showStrategy())}>
              <Chevron open={showStrategy()} />
              脚本编辑器
            </button>
            <Show when={showStrategy()}>
              <div class="px-5 pb-4 space-y-3">
                <Show when={savedStrategies().length > 0}>
                  <select class={sel} onChange={e => handleSelectStrategy(e.currentTarget.value)}>
                    <option value="">从已保存策略加载...</option>
                    <For each={savedStrategies()}>{(s) => <option value={s.id}>{s.name}</option>}</For>
                  </select>
                </Show>
                <div>
                  <div class="flex items-center justify-between mb-1.5">
                    <label class="text-[11px] font-medium text-slate-400 tracking-wide uppercase">Lua 脚本</label>
                    <button type="button" class="text-[11px] font-medium text-indigo-500 hover:text-indigo-600 disabled:opacity-50 transition-colors" disabled={scriptValidating()} onClick={handleValidateScript}>
                      {scriptValidating() ? '验证中...' : '验证语法'}
                    </button>
                  </div>
                  <textarea class="font-mono text-[13px] leading-relaxed bg-slate-900 text-slate-100 rounded-xl p-4 min-h-[180px] w-full resize-y focus:outline-none focus:ring-2 focus:ring-indigo-500/30 border border-slate-700/50 selection:bg-indigo-500/30"
                    value={strategyCode()} onInput={e => { setStrategyCode(e.currentTarget.value); setScriptValidationResult(null) }} placeholder="在此编写 Lua 策略脚本..." />
                  <Show when={scriptValidationResult()}>
                    <p class={`text-[11px] mt-1.5 font-medium ${scriptValidationResult()!.valid ? 'text-emerald-500' : 'text-red-400'}`}>
                      {scriptValidationResult()!.valid ? '✓ 语法正确' : `✗ ${scriptValidationResult()!.error}`}
                    </p>
                  </Show>
                </div>
                <div>
                  <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">脚本参数 (JSON)</label>
                  <textarea class="w-full bg-slate-50 border border-slate-200 rounded-lg px-3 py-2 text-[13px] font-mono text-slate-800 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-300 focus:bg-white transition-all" rows={2}
                    value={scriptParams()} onInput={e => setScriptParams(e.currentTarget.value)} placeholder='{"fast_period": 12}' />
                </div>
              </div>
            </Show>
          </div>
        </Show>

        {/* 错误 */}
        <Show when={runError()}>
          <div class="mx-5 mb-4 p-3 bg-red-50 border border-red-200/60 rounded-lg text-[13px] text-red-600 flex items-center gap-2">
            <svg class="w-4 h-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" /></svg>
            {runError()}
          </div>
        </Show>
      </div>

      {/* ═══ 回测结果 ═══ */}
      <Show when={result()}>
        <div id="backtest-result" class="space-y-4">
          {/* 标题 + AI */}
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-3">
              <div class="w-1 h-6 bg-indigo-500 rounded-full" />
              <h3 class="text-[15px] font-semibold text-slate-800">
                {result()!.strategy_name} <span class="text-slate-400 font-normal">/ {result()!.symbol} · {result()!.exchange}</span>
              </h3>
            </div>
            <button class="px-4 py-1.5 bg-gradient-to-r from-violet-500 to-indigo-500 text-white rounded-lg text-[12px] font-semibold hover:from-violet-600 hover:to-indigo-600 transition-all disabled:opacity-50 shadow-sm shadow-violet-200/50"
              disabled={aiLoading()} onClick={handleAiOptimize}>
              {aiLoading() ? '分析中...' : '✨ AI 优化建议'}
            </button>
          </div>

          {/* AI 建议 */}
          <Show when={aiSuggestion()}>
            <div class="p-4 bg-gradient-to-br from-violet-50/80 via-indigo-50/50 to-slate-50 rounded-xl border border-violet-100/60 text-[13px] text-slate-700 whitespace-pre-wrap max-h-48 overflow-y-auto leading-relaxed" innerHTML={aiSuggestion()!} />
          </Show>

          {/* 核心指标 — 英雄卡片 */}
          <div class="grid grid-cols-2 lg:grid-cols-4 gap-3">
            {/* 总收益率 — 突出显示 */}
            <div class={`col-span-2 lg:col-span-1 rounded-xl p-4 ${result()!.total_return_pct >= 0 ? 'bg-emerald-50 border border-emerald-200/60' : 'bg-red-50 border border-red-200/60'}`}>
              <p class="text-[11px] font-medium text-slate-500 uppercase tracking-wider mb-1">总收益率</p>
              <p class={`text-2xl font-bold tracking-tight ${result()!.total_return_pct >= 0 ? 'text-emerald-600' : 'text-red-500'}`}>{fmtPct(result()!.total_return_pct)}</p>
              <p class="text-[11px] text-slate-400 mt-1">{fmt(result()!.initial_balance)} → {fmt(result()!.final_balance)}</p>
            </div>
            {/* 最大回撤 */}
            <div class="bg-white rounded-xl border border-slate-200/80 p-4">
              <p class="text-[11px] font-medium text-slate-500 uppercase tracking-wider mb-1">最大回撤</p>
              <p class="text-2xl font-bold text-red-500 tracking-tight">{fmtPct(-Math.abs(result()!.max_drawdown_pct))}</p>
            </div>
            {/* 夏普 */}
            <div class="bg-white rounded-xl border border-slate-200/80 p-4">
              <p class="text-[11px] font-medium text-slate-500 uppercase tracking-wider mb-1">夏普比率</p>
              <p class="text-2xl font-bold text-slate-800 tracking-tight">{fmt(result()!.sharpe_ratio)}</p>
            </div>
            {/* 胜率 */}
            <div class="bg-white rounded-xl border border-slate-200/80 p-4">
              <p class="text-[11px] font-medium text-slate-500 uppercase tracking-wider mb-1">胜率</p>
              <p class="text-2xl font-bold text-slate-800 tracking-tight">{fmt(result()!.win_rate)}%</p>
              <div class="mt-1.5 h-1.5 bg-slate-100 rounded-full overflow-hidden">
                <div class="h-full bg-indigo-500 rounded-full transition-all" style={{ width: `${Math.min(100, result()!.win_rate)}%` }} />
              </div>
            </div>
          </div>

          {/* 次要指标 */}
          <div class="grid grid-cols-3 md:grid-cols-6 gap-2.5">
            {[
              { label: '索提诺', value: fmt(result()!.sortino_ratio), color: '' },
              { label: '盈亏比', value: fmt(result()!.profit_factor), color: '' },
              { label: '总交易', value: String(result()!.total_trades), color: '' },
              { label: '盈利', value: String(result()!.profit_trades), color: 'text-emerald-600' },
              { label: '亏损', value: String(result()!.loss_trades), color: 'text-red-500' },
              { label: '均盈/均亏', value: `${fmt(result()!.avg_profit)}/${fmt(result()!.avg_loss)}`, color: '' },
            ].map(m => (
              <div class="bg-white rounded-lg border border-slate-200/80 px-3 py-2.5">
                <p class="text-[10px] text-slate-400 uppercase tracking-wider">{m.label}</p>
                <p class={`text-[15px] font-semibold text-slate-800 mt-0.5 ${m.color}`}>{m.value}</p>
              </div>
            ))}
          </div>

          {/* 图表区域 */}
          <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
            <Show when={result()?.trades && result()!.trades.length > 0}>
              <div class="bg-white rounded-xl border border-slate-200/80 p-4">
                <h4 class="text-[12px] font-semibold text-slate-500 uppercase tracking-wider mb-3">交易信号</h4>
                <KlineChart data={backtestKlineData()} height={280} markers={backtestMarkers()} overlays={backtestOverlays()} />
              </div>
            </Show>
            <Show when={result()?.equity_curve && result()!.equity_curve.length > 0}>
              <div class="bg-white rounded-xl border border-slate-200/80 p-4">
                <h4 class="text-[12px] font-semibold text-slate-500 uppercase tracking-wider mb-3">权益曲线</h4>
                <EquityChart data={result()!.equity_curve} height={280} initialBalance={result()!.initial_balance} />
              </div>
            </Show>
          </div>

          {/* 交易记录 */}
          <div class="bg-white rounded-xl border border-slate-200/80 overflow-hidden">
            <div class="px-4 py-3 border-b border-slate-100 flex items-center justify-between">
              <h4 class="text-[12px] font-semibold text-slate-500 uppercase tracking-wider">交易记录</h4>
              <span class="text-[11px] text-slate-400">{result()!.trades.length} 笔</span>
            </div>
            <div class="overflow-x-auto max-h-[360px] overflow-y-auto">
              <table class="w-full text-[13px]">
                <thead class="sticky top-0 bg-slate-50/95 backdrop-blur-sm">
                  <tr class="border-b border-slate-100">
                    {['入场', '出场', '方向', '入场价', '出场价', '数量', '盈亏', '盈亏%', '手续费'].map(h => (
                      <th class={`py-2.5 px-3 text-[11px] font-semibold text-slate-400 uppercase tracking-wider ${['入场价','出场价','数量','盈亏','盈亏%','手续费'].includes(h) ? 'text-right' : 'text-left'}`}>{h}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  <For each={result()!.trades}>
                    {(trade, i) => (
                      <tr class={`border-b border-slate-50 hover:bg-slate-50/50 transition-colors ${i() % 2 === 0 ? '' : 'bg-slate-50/30'}`}>
                        <td class="py-2 px-3 text-slate-500 text-[12px] font-mono">{fmtDT(trade.entry_time)}</td>
                        <td class="py-2 px-3 text-slate-500 text-[12px] font-mono">{fmtDT(trade.exit_time)}</td>
                        <td class="py-2 px-3">
                          <span class={`inline-flex items-center px-2 py-[2px] rounded-md text-[11px] font-semibold ${trade.side === 'long' ? 'bg-emerald-50 text-emerald-600' : 'bg-red-50 text-red-500'}`}>
                            {trade.side === 'long' ? '多' : '空'}
                          </span>
                        </td>
                        <td class="py-2 px-3 text-right text-slate-600 font-mono text-[12px]">{fmt(trade.entry_price)}</td>
                        <td class="py-2 px-3 text-right text-slate-600 font-mono text-[12px]">{fmt(trade.exit_price)}</td>
                        <td class="py-2 px-3 text-right text-slate-500 font-mono text-[12px]">{fmt(trade.quantity, 4)}</td>
                        <td class={`py-2 px-3 text-right font-semibold text-[12px] font-mono ${trade.pnl >= 0 ? 'text-emerald-600' : 'text-red-500'}`}>{fmt(trade.pnl)}</td>
                        <td class={`py-2 px-3 text-right font-semibold text-[12px] font-mono ${trade.pnl_pct >= 0 ? 'text-emerald-600' : 'text-red-500'}`}>{fmtPct(trade.pnl_pct)}</td>
                        <td class="py-2 px-3 text-right text-slate-400 font-mono text-[12px]">{fmt(trade.commission)}</td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </div>
          </div>

          {/* 资金费率 */}
          <Show when={result()?.funding_events && result()!.funding_events!.length > 0}>
            <div class="bg-white rounded-xl border border-slate-200/80 overflow-hidden">
              <div class="px-4 py-3 border-b border-slate-100 flex items-center justify-between">
                <h4 class="text-[12px] font-semibold text-slate-500 uppercase tracking-wider">资金费率</h4>
                <span class="text-[11px] font-medium">
                  总计: <span class={result()!.funding_events!.reduce((s, e) => s + e.amount, 0) >= 0 ? 'text-emerald-600' : 'text-red-500'}>
                    {result()!.funding_events!.reduce((s, e) => s + e.amount, 0) >= 0 ? '+' : ''}{fmt(result()!.funding_events!.reduce((s, e) => s + e.amount, 0))}
                  </span>
                </span>
              </div>
              <div class="overflow-x-auto max-h-[200px] overflow-y-auto">
                <table class="w-full text-[13px]">
                  <thead class="sticky top-0 bg-slate-50/95 backdrop-blur-sm">
                    <tr class="border-b border-slate-100">
                      <th class="text-left py-2 px-3 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">时间</th>
                      <th class="text-left py-2 px-3 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">方向</th>
                      <th class="text-right py-2 px-3 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">费率</th>
                      <th class="text-right py-2 px-3 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">金额</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={result()!.funding_events}>
                      {(event) => (
                        <tr class="border-b border-slate-50 hover:bg-slate-50/50 transition-colors">
                          <td class="py-1.5 px-3 text-slate-500 text-[12px] font-mono">{new Date(event.time).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })}</td>
                          <td class="py-1.5 px-3">
                            <span class={`inline-flex items-center px-2 py-[2px] rounded-md text-[11px] font-semibold ${event.side === 'long' ? 'bg-emerald-50 text-emerald-600' : 'bg-red-50 text-red-500'}`}>
                              {event.side === 'long' ? '多' : '空'}
                            </span>
                          </td>
                          <td class="py-1.5 px-3 text-right text-slate-400 font-mono text-[12px]">{(event.rate * 100).toFixed(4)}%</td>
                          <td class={`py-1.5 px-3 text-right font-semibold text-[12px] font-mono ${event.amount > 0 ? 'text-emerald-600' : event.amount < 0 ? 'text-red-500' : 'text-slate-400'}`}>
                            {event.amount > 0 ? '+' : ''}{fmt(event.amount)}
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </div>
            </div>
          </Show>
        </div>
      </Show>

      {/* ═══ 历史回测 ═══ */}
      <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
        <div class="px-5 py-4 border-b border-slate-100">
          <h3 class="text-[14px] font-semibold text-slate-800">历史回测</h3>
        </div>
        <Show when={!historyLoading()} fallback={<div class="flex items-center justify-center py-12"><div class="w-5 h-5 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" /></div>}>
          <Show when={historyError() === ''} fallback={<div class="text-center py-12"><p class="text-[13px] text-red-400">{historyError()}</p><button class="mt-2 text-[13px] text-indigo-600 hover:text-indigo-700 font-medium" onClick={() => loadHistory(historyPage())}>重试</button></div>}>
            <Show when={historyItems().length > 0} fallback={<div class="text-center py-12"><p class="text-slate-400 text-[13px]">暂无历史回测记录</p></div>}>
              <div class="overflow-x-auto">
                <table class="w-full text-[13px]">
                  <thead>
                    <tr class="border-b border-slate-100 bg-slate-50/50">
                      <th class="text-left py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">策略</th>
                      <th class="text-left py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">交易对</th>
                      <th class="text-right py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">初始</th>
                      <th class="text-right py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">最终</th>
                      <th class="text-right py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">收益率</th>
                      <th class="text-right py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">夏普</th>
                      <th class="text-right py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">回撤</th>
                      <th class="text-center py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={historyItems()}>
                      {(item, i) => (
                        <tr class={`border-b border-slate-50 hover:bg-indigo-50/30 transition-colors cursor-pointer ${i() % 2 === 0 ? '' : 'bg-slate-50/20'}`} onClick={() => handleViewDetail(item.id)}>
                          <td class="py-2.5 px-4 text-slate-700 font-medium">{item.strategy_name}</td>
                          <td class="py-2.5 px-4 text-slate-500">{item.symbol}</td>
                          <td class="py-2.5 px-4 text-right text-slate-500 font-mono">{fmt(item.initial_balance)}</td>
                          <td class="py-2.5 px-4 text-right text-slate-700 font-mono font-medium">{fmt(item.final_balance)}</td>
                          <td class={`py-2.5 px-4 text-right font-semibold font-mono ${item.total_return_pct >= 0 ? 'text-emerald-600' : 'text-red-500'}`}>{fmtPct(item.total_return_pct)}</td>
                          <td class="py-2.5 px-4 text-right text-slate-500 font-mono">{fmt(item.sharpe_ratio)}</td>
                          <td class="py-2.5 px-4 text-right text-red-400 font-mono">{fmtPct(-Math.abs(item.max_drawdown_pct))}</td>
                          <td class="py-2.5 px-4 text-center">
                            <span class="text-indigo-500 hover:text-indigo-600 text-[12px] font-semibold">详情 →</span>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </div>
              <Show when={historyTotalPages() > 1}>
                <div class="flex items-center justify-between px-5 py-3 border-t border-slate-100">
                  <p class="text-[12px] text-slate-400">{historyPage()} / {historyTotalPages()}</p>
                  <div class="flex gap-2">
                    <button class="px-3 py-1 text-[12px] border border-slate-200 rounded-lg text-slate-500 hover:bg-slate-50 hover:text-slate-700 disabled:opacity-30 transition-colors font-medium" disabled={historyPage() <= 1} onClick={() => loadHistory(historyPage() - 1)}>上一页</button>
                    <button class="px-3 py-1 text-[12px] border border-slate-200 rounded-lg text-slate-500 hover:bg-slate-50 hover:text-slate-700 disabled:opacity-30 transition-colors font-medium" disabled={historyPage() >= historyTotalPages()} onClick={() => loadHistory(historyPage() + 1)}>下一页</button>
                  </div>
                </div>
              </Show>
            </Show>
          </Show>
        </Show>
      </div>
    </div>
  )
}

export default Backtest
