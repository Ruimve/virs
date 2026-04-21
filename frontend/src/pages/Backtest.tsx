import { type Component, createSignal, createMemo, Show, For, onMount } from 'solid-js'
import { api, fetchPlugins, validateScript, type PaginatedResponse, type Plugin } from '../lib/api'
import KlineChart from '../components/KlineChart'
import EquityChart from '../components/EquityChart'
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
  side: 'buy' | 'sell'
  entry_price: number
  exit_price: number
  quantity: number
  pnl: number
  pnl_pct: number
  commission: number
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

// ---- 默认指标配置 ----
// Indicator plugins are loaded from the backend at runtime.
// Users configure indicator parameters through the UI.

const DEFAULT_TRADING_CONFIG: Record<string, unknown> = {
  stop_loss_pct: 0.03,
  take_profit_pct: 0.06,
  commission_rate: 0.001,
  slippage: 0.0005,
  position_pct: 1.0,
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

function formatNumber(n: number | undefined | null, decimals = 2): string {
  if (n == null || isNaN(n)) return '-'
  return n.toFixed(decimals)
}

function formatPct(n: number | undefined | null): string {
  if (n == null || isNaN(n)) return '-'
  return `${n >= 0 ? '+' : ''}${formatNumber(n)}%`
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

// ---- 组件 ----

const Backtest: Component = () => {
  // 插件列表状态
  const [plugins, setPlugins] = createSignal<Plugin[]>([])
  const [pluginsLoading, setPluginsLoading] = createSignal(false)
  const [pluginsError, setPluginsError] = createSignal('')

  // 表单状态
  const [backtestMode, setBacktestMode] = createSignal<'plugin' | 'script'>('plugin')
  const [strategyType, setStrategyType] = createSignal('custom')
  const [symbol, setSymbol] = createSignal('BTCUSDT')
  const [exchange, setExchange] = createSignal('binance')
  const [timeframe, setTimeframe] = createSignal('1h')
  const [startDate, setStartDate] = createSignal((() => { const d = new Date(); d.setFullYear(d.getFullYear() - 1); return d.toISOString().split('T')[0] })())
  const [endDate, setEndDate] = createSignal(new Date().toISOString().split('T')[0])
  const [initialBalance, setInitialBalance] = createSignal(10000)
  const [indicatorConfig, setIndicatorConfig] = createSignal('{}')
  const [tradingConfig, setTradingConfig] = createSignal(
    JSON.stringify(DEFAULT_TRADING_CONFIG, null, 2)
  )
  const [strategyCode, setStrategyCode] = createSignal(DEFAULT_LUA_SCRIPT)
  const [scriptParams, setScriptParams] = createSignal('{"fast_period": 12, "slow_period": 26, "rsi_period": 14, "rsi_floor": 45}')

  // 运行状态
  const [running, setRunning] = createSignal(false)
  const [runError, setRunError] = createSignal('')

  // 脚本验证状态
  const [scriptValidating, setScriptValidating] = createSignal(false)
  const [scriptValidationResult, setScriptValidationResult] = createSignal<{ valid: boolean; error?: string } | null>(null)

  // 结果状态
  const [result, setResult] = createSignal<BacktestResult | null>(null)

  // 历史列表状态
  const [historyItems, setHistoryItems] = createSignal<BacktestSummary[]>([])
  const [historyPage, setHistoryPage] = createSignal(1)
  const [historyTotalPages, setHistoryTotalPages] = createSignal(0)
  const [historyLoading, setHistoryLoading] = createSignal(false)
  const [historyError, setHistoryError] = createSignal('')

  // ── 加载插件列表 ──
  async function loadPlugins() {
    setPluginsLoading(true)
    setPluginsError('')
    try {
      const res = await fetchPlugins()
      if (res.success && res.data) {
        setPlugins(res.data)
        // 自动选中第一个插件
        if (res.data.length > 0) {
          const first = res.data[0]
          setStrategyType(first.name)
          setIndicatorConfig(JSON.stringify(buildIndicatorConfig(first), null, 2))
        }
      } else {
        setPluginsError(res.error || '加载插件列表失败')
      }
    } catch (e) {
      setPluginsError(e instanceof Error ? e.message : '加载插件列表失败')
    } finally {
      setPluginsLoading(false)
    }
  }

  // 根据插件参数构建默认指标配置
  function buildIndicatorConfig(plugin: Plugin): Record<string, unknown> {
    const config: Record<string, unknown> = { plugin: plugin.name }
    for (const param of plugin.params) {
      config[param.name] = param.default
    }
    return config
  }

  // 策略类型变化时更新指标配置
  function handleStrategyTypeChange(pluginName: string) {
    setStrategyType(pluginName)
    const plugin = plugins().find((p) => p.name === pluginName)
    if (plugin) {
      setIndicatorConfig(JSON.stringify(buildIndicatorConfig(plugin), null, 2))
    }
  }

  // 验证 Lua 脚本
  async function handleValidateScript() {
    setScriptValidating(true)
    setScriptValidationResult(null)
    try {
      const res = await validateScript(strategyCode())
      if (res.success && res.data) {
        setScriptValidationResult(res.data)
      } else {
        setScriptValidationResult({ valid: false, error: res.error || '验证请求失败' })
      }
    } catch (e) {
      setScriptValidationResult({ valid: false, error: e instanceof Error ? e.message : '网络错误' })
    } finally {
      setScriptValidating(false)
    }
  }

  // 加载历史列表
  async function loadHistory(page: number) {
    setHistoryLoading(true)
    setHistoryError('')
    try {
      const res = await api.get<PaginatedResponse<BacktestSummary>>(
        `/backtest/list?page=${page}&page_size=10`
      )
      if (res.success && res.data) {
        setHistoryItems(res.data.items)
        setHistoryTotalPages(res.data.total_pages)
        setHistoryPage(res.data.page)
      } else {
        setHistoryError(res.error || '加载历史回测列表失败')
      }
    } catch (e) {
      setHistoryError(e instanceof Error ? e.message : '加载历史回测列表失败')
    } finally {
      setHistoryLoading(false)
    }
  }

  onMount(() => {
    loadPlugins()
    loadHistory(1)
  })

  // 运行回测
  async function handleRun() {
    setRunning(true)
    setRunError('')
    setResult(null)

    let parsedIndicator: Record<string, unknown>
    let parsedTrading: Record<string, unknown>

    try {
      parsedIndicator = JSON.parse(indicatorConfig())
    } catch {
      setRunError('指标配置 JSON 格式错误')
      setRunning(false)
      return
    }

    try {
      parsedTrading = JSON.parse(tradingConfig())
    } catch {
      setRunError('交易配置 JSON 格式错误')
      setRunning(false)
      return
    }

    // 脚本模式: 将 strategy_code 和 params 注入 indicator_config
    if (backtestMode() === 'script') {
      let parsedParams: Record<string, unknown> = {}
      try {
        parsedParams = JSON.parse(scriptParams())
      } catch {
        setRunError('脚本参数 JSON 格式错误')
        setRunning(false)
        return
      }
      parsedIndicator = {
        ...parsedParams,
        ...parsedIndicator,
        strategy_code: strategyCode(),
      }
    }

    const req: BacktestRequest = {
      strategy_type: backtestMode() === 'script' ? 'script' : strategyType(),
      symbol: symbol(),
      exchange: exchange(),
      timeframe: timeframe(),
      start_date: startDate() || null,
      end_date: endDate() || null,
      initial_balance: initialBalance(),
      indicator_config: parsedIndicator,
      trading_config: parsedTrading,
    }

    try {
      const res = await api.post<BacktestResult>('/backtest/run', req)
      if (res.success && res.data) {
        setResult(res.data)
      } else {
        setRunError(res.error || '回测运行失败')
      }
    } catch (e) {
      setRunError(e instanceof Error ? e.message : '回测运行失败')
    } finally {
      setRunning(false)
    }
  }

  // 查看历史详情
  async function handleViewDetail(id: string) {
    setResult(null)
    setRunError('')
    try {
      const res = await api.get<BacktestResult>(`/backtest/${id}`)
      if (res.success && res.data) {
        setResult(res.data)
        // 滚动到结果区域
        const el = document.getElementById('backtest-result')
        if (el) {
          el.scrollIntoView({ behavior: 'smooth', block: 'start' })
        }
      } else {
        setRunError(res.error || '加载回测详情失败')
      }
    } catch (e) {
      setRunError(e instanceof Error ? e.message : '加载回测详情失败')
    }
  }

  // 从回测结果中提取 K 线数据（如果有）
  const backtestKlineData = createMemo(() => {
    const r = result()
    if (!r || !r.trades || r.trades.length === 0) return []
    // 从交易记录中生成简化的 K 线数据
    // 每笔交易生成一个数据点
    return r.trades.map((t) => ({
      time: Math.floor(new Date(t.entry_time).getTime() / 1000),
      open: t.entry_price,
      high: Math.max(t.entry_price, t.exit_price),
      low: Math.min(t.entry_price, t.exit_price),
      close: t.exit_price,
    }))
  })

  const backtestMarkers = createMemo(() => {
    const r = result()
    if (!r || !r.trades) return []
    return r.trades.flatMap((t) => {
      const markers = []
      markers.push({
        time: Math.floor(new Date(t.entry_time).getTime() / 1000),
        position: (t.side === 'buy' ? 'belowBar' : 'aboveBar') as 'belowBar' | 'aboveBar',
        color: t.side === 'buy' ? '#10b981' : '#ef4444',
        shape: (t.side === 'buy' ? 'arrowUp' : 'arrowDown') as 'arrowUp' | 'arrowDown',
        text: t.side === 'buy' ? 'Buy' : 'Sell',
      })
      markers.push({
        time: Math.floor(new Date(t.exit_time).getTime() / 1000),
        position: (t.side === 'buy' ? 'aboveBar' : 'belowBar') as 'aboveBar' | 'belowBar',
        color: t.side === 'buy' ? '#ef4444' : '#10b981',
        shape: (t.side === 'buy' ? 'arrowDown' : 'arrowUp') as 'arrowDown' | 'arrowUp',
        text: `Exit (${t.pnl >= 0 ? '+' : ''}${formatNumber(t.pnl_pct, 1)}%)`,
      })
      return markers
    })
  })

  // 计算回测 K 线的指标叠加
  const backtestOverlays = createMemo(() => {
    const data = backtestKlineData()
    if (!data || data.length < 2) return []

    const closes = data.map(k => k.close)
    const overlays: OverlayLine[] = []

    // SMA 20
    const sma20 = computeSMA(closes, 20)
    overlays.push({
      name: 'SMA 20',
      data: sma20.map((v, i) => ({ time: data[i].time, value: v })).filter(d => d.value !== null) as Array<{ time: number; value: number }>,
      color: '#f59e0b',
      lineWidth: 1,
    })

    // EMA 12
    const ema12 = computeEMA(closes, 12)
    overlays.push({
      name: 'EMA 12',
      data: ema12.map((v, i) => ({ time: data[i].time, value: v })).filter(d => d.value !== null) as Array<{ time: number; value: number }>,
      color: '#6366f1',
      lineWidth: 1,
    })

    return overlays
  })

  return (
    <div class="space-y-6">
      {/* 页头 */}
      <div>
        <h2 class="text-lg font-semibold text-gray-900">回测</h2>
        <p class="text-sm text-gray-400 mt-1">对策略进行历史数据回测</p>
      </div>

      {/* 回测配置表单 */}
      <div class="bg-white rounded-xl border border-gray-200/60 p-6">
        <h3 class="text-[15px] font-semibold text-gray-800 mb-5">回测配置</h3>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {/* 模式切换 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">策略模式</label>
            <div class="flex gap-1 bg-gray-100 rounded-lg p-1 w-fit">
              <button
                type="button"
                class={`px-4 py-1.5 text-sm rounded-md transition-colors ${
                  backtestMode() === 'plugin'
                    ? 'bg-indigo-600 text-white font-medium'
                    : 'bg-gray-100 text-gray-600 hover:text-gray-800'
                }`}
                onClick={() => setBacktestMode('plugin')}
              >
                插件模式
              </button>
              <button
                type="button"
                class={`px-4 py-1.5 text-sm rounded-md transition-colors ${
                  backtestMode() === 'script'
                    ? 'bg-indigo-600 text-white font-medium'
                    : 'bg-gray-100 text-gray-600 hover:text-gray-800'
                }`}
                onClick={() => {
                  setBacktestMode('script')
                  setScriptValidationResult(null)
                }}
              >
                脚本模式
              </button>
            </div>
          </div>

          {/* 插件模式: 策略类型 */}
          <Show when={backtestMode() === 'plugin'}>
            <div>
              <label class="block text-[13px] font-medium text-gray-400 mb-1.5">策略类型</label>
              <Show
                when={!pluginsLoading()}
                fallback={
                  <select
                    class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 bg-white"
                    disabled
                  >
                    <option>加载中...</option>
                  </select>
                }
              >
                <Show
                  when={pluginsError() === ''}
                  fallback={
                    <div>
                      <select
                        class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 bg-white"
                        disabled
                      >
                        <option>加载失败</option>
                      </select>
                      <p class="text-xs text-red-500 mt-1">{pluginsError()}</p>
                    </div>
                  }
                >
                  <select
                    class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 bg-white"
                    value={strategyType()}
                    onChange={(e) => handleStrategyTypeChange(e.currentTarget.value)}
                  >
                    <For each={plugins()}>
                      {(plugin) => (
                        <option value={plugin.name}>
                          {plugin.name} - {plugin.description.slice(0, 20)}
                        </option>
                      )}
                    </For>
                  </select>
                </Show>
              </Show>
            </div>
          </Show>

          {/* 交易对 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">交易对</label>
            <input
              type="text"
              class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 placeholder:text-gray-300"
              value={symbol()}
              onInput={(e) => setSymbol(e.currentTarget.value)}
              placeholder="BTCUSDT"
            />
          </div>

          {/* 交易所 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">交易所</label>
            <select
              class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 bg-white"
              value={exchange()}
              onChange={(e) => setExchange(e.currentTarget.value)}
            >
              <option value="binance">Binance</option>
              <option value="okx">OKX</option>
              <option value="bybit">Bybit</option>
            </select>
          </div>

          {/* 时间周期 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">时间周期</label>
            <select
              class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 bg-white"
              value={timeframe()}
              onChange={(e) => setTimeframe(e.currentTarget.value)}
            >
              <option value="1m">1 分钟</option>
              <option value="5m">5 分钟</option>
              <option value="15m">15 分钟</option>
              <option value="1h">1 小时</option>
              <option value="4h">4 小时</option>
              <option value="1d">1 天</option>
            </select>
          </div>

          {/* 开始日期 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">开始日期</label>
            <input
              type="date"
              class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
              value={startDate()}
              onInput={(e) => setStartDate(e.currentTarget.value)}
            />
          </div>

          {/* 结束日期 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">结束日期</label>
            <input
              type="date"
              class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
              value={endDate()}
              onInput={(e) => setEndDate(e.currentTarget.value)}
            />
          </div>

          {/* 初始资金 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">初始资金 (USDT)</label>
            <input
              type="number"
              class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
              value={initialBalance()}
              onInput={(e) => setInitialBalance(Number(e.currentTarget.value))}
              min="0"
              step="100"
            />
          </div>

          {/* 指标配置 */}
          <div class="md:col-span-1 lg:col-span-1">
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">指标配置 (JSON)</label>
            <textarea
              class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm font-mono text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
              rows={4}
              value={indicatorConfig()}
              onInput={(e) => setIndicatorConfig(e.currentTarget.value)}
            />
          </div>

          {/* 交易配置 */}
          <div class="md:col-span-2 lg:col-span-2">
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">交易配置</label>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
              <div>
                <label class="block text-xs text-gray-500 mb-1">止损 (%)</label>
                <input
                  type="number"
                  step="0.01"
                  min="0"
                  class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
                  value={(() => { try { return JSON.parse(tradingConfig()).stop_loss_pct ?? '' } catch { return '' } })()}
                  onInput={(e) => {
                    const val = parseFloat(e.currentTarget.value)
                    const updated = { ...JSON.parse(tradingConfig()), stop_loss_pct: isNaN(val) ? null : val }
                    setTradingConfig(JSON.stringify(updated, null, 2))
                  }}
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">止盈 (%)</label>
                <input
                  type="number"
                  step="0.01"
                  min="0"
                  class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
                  value={(() => { try { return JSON.parse(tradingConfig()).take_profit_pct ?? '' } catch { return '' } })()}
                  onInput={(e) => {
                    const val = parseFloat(e.currentTarget.value)
                    const updated = { ...JSON.parse(tradingConfig()), take_profit_pct: isNaN(val) ? null : val }
                    setTradingConfig(JSON.stringify(updated, null, 2))
                  }}
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">手续费 (%)</label>
                <input
                  type="number"
                  step="0.001"
                  min="0"
                  class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
                  value={(() => { try { return JSON.parse(tradingConfig()).commission_rate ?? '' } catch { return '' } })()}
                  onInput={(e) => {
                    const val = parseFloat(e.currentTarget.value)
                    const updated = { ...JSON.parse(tradingConfig()), commission_rate: isNaN(val) ? null : val }
                    setTradingConfig(JSON.stringify(updated, null, 2))
                  }}
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">滑点 (%)</label>
                <input
                  type="number"
                  step="0.0001"
                  min="0"
                  class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
                  value={(() => { try { return JSON.parse(tradingConfig()).slippage ?? '' } catch { return '' } })()}
                  onInput={(e) => {
                    const val = parseFloat(e.currentTarget.value)
                    const updated = { ...JSON.parse(tradingConfig()), slippage: isNaN(val) ? null : val }
                    setTradingConfig(JSON.stringify(updated, null, 2))
                  }}
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">仓位比例 (0-1)</label>
                <input
                  type="number"
                  step="0.1"
                  min="0.01"
                  max="1"
                  class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
                  value={(() => { try { return JSON.parse(tradingConfig()).position_pct ?? '' } catch { return '' } })()}
                  onInput={(e) => {
                    const val = parseFloat(e.currentTarget.value)
                    const updated = { ...JSON.parse(tradingConfig()), position_pct: isNaN(val) ? 1.0 : Math.min(1, Math.max(0.01, val)) }
                    setTradingConfig(JSON.stringify(updated, null, 2))
                  }}
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">交易方向</label>
                <select
                  class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
                  value={(() => { try { return JSON.parse(tradingConfig()).trade_direction ?? 'long' } catch { return 'long' } })()}
                  onInput={(e) => {
                    const updated = { ...JSON.parse(tradingConfig()), trade_direction: e.currentTarget.value }
                    setTradingConfig(JSON.stringify(updated, null, 2))
                  }}
                >
                  <option value="long">仅做多</option>
                  <option value="short">仅做空</option>
                  <option value="both">多空双向</option>
                </select>
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">追踪止损 (%)</label>
                <input
                  type="number"
                  step="0.01"
                  min="0"
                  placeholder="关闭"
                  class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
                  onInput={(e) => {
                    const val = e.currentTarget.value
                    const updated = { ...JSON.parse(tradingConfig()), trailing_stop_pct: val === '' ? null : parseFloat(val) }
                    setTradingConfig(JSON.stringify(updated, null, 2))
                  }}
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">追踪激活 (%)</label>
                <input
                  type="number"
                  step="0.01"
                  min="0"
                  placeholder="关闭"
                  class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
                  onInput={(e) => {
                    const val = e.currentTarget.value
                    const updated = { ...JSON.parse(tradingConfig()), trailing_activation_pct: val === '' ? null : parseFloat(val) }
                    setTradingConfig(JSON.stringify(updated, null, 2))
                  }}
                />
              </div>
            </div>
          </div>
        </div>

        {/* 脚本模式: Lua 编辑器 + 参数 */}
        <Show when={backtestMode() === 'script'}>
          <div class="mt-5 space-y-4">
            {/* Lua 脚本 */}
            <div>
              <div class="flex items-center justify-between mb-1.5">
                <label class="block text-[13px] font-medium text-gray-400">Lua 脚本</label>
                <button
                  type="button"
                  class="text-sm text-indigo-600 hover:text-indigo-700 disabled:opacity-50"
                  disabled={scriptValidating()}
                  onClick={handleValidateScript}
                >
                  {scriptValidating() ? '验证中...' : '验证脚本'}
                </button>
              </div>
              <textarea
                class="font-mono text-sm bg-gray-900 text-gray-100 rounded-lg p-4 min-h-[200px] w-full resize-y focus:outline-none focus:ring-2 focus:ring-indigo-500/30 border border-gray-700"
                value={strategyCode()}
                onInput={(e) => {
                  setStrategyCode(e.currentTarget.value)
                  setScriptValidationResult(null)
                }}
                placeholder="在此编写 Lua 策略脚本..."
              />
              <Show when={scriptValidationResult()}>
                <div class="mt-2">
                  <Show
                    when={scriptValidationResult()!.valid}
                    fallback={
                      <p class="text-sm text-red-500">{scriptValidationResult()!.error}</p>
                    }
                  >
                    <p class="text-sm text-emerald-600">{'\u2713'} 脚本语法正确</p>
                  </Show>
                </div>
              </Show>
            </div>

            {/* 脚本参数 */}
            <div>
              <label class="block text-[13px] font-medium text-gray-400 mb-1.5">脚本参数 (JSON)</label>
              <textarea
                class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm font-mono text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
                rows={3}
                value={scriptParams()}
                onInput={(e) => setScriptParams(e.currentTarget.value)}
                placeholder='{"fast_period": 12, "slow_period": 26}'
              />
            </div>
          </div>
        </Show>

        {/* 错误信息 */}
        <Show when={runError()}>
          <div class="mt-4 p-3 bg-red-50/80 border border-red-200/60 rounded-lg text-sm text-red-500">
            {runError()}
          </div>
        </Show>

        {/* 运行按钮 */}
        <div class="mt-5">
          <button
            class="px-5 py-2 bg-indigo-600 text-white text-sm font-medium rounded-lg hover:bg-indigo-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            disabled={running()}
            onClick={handleRun}
          >
            {running() ? '运行中...' : '运行回测'}
          </button>
        </div>
      </div>

      {/* 回测结果 */}
      <Show when={result()}>
        <div id="backtest-result" class="space-y-5">
          <h3 class="text-[15px] font-semibold text-gray-800">
            回测结果 - {result()!.strategy_name} / {result()!.symbol} ({result()!.exchange})
          </h3>

          {/* 8 个统计卡片 */}
          <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
            {/* 总收益率 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">总收益率</p>
              <p
                class={`text-xl font-bold ${
                  result()!.total_return_pct >= 0 ? 'text-emerald-600' : 'text-red-500'
                }`}
              >
                {formatPct(result()!.total_return_pct)}
              </p>
            </div>

            {/* 最大回撤 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">最大回撤</p>
              <p class="text-xl font-bold text-red-500">
                {formatPct(-Math.abs(result()!.max_drawdown_pct))}
              </p>
            </div>

            {/* 夏普比率 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">夏普比率</p>
              <p class="text-xl font-bold text-gray-900">
                {formatNumber(result()!.sharpe_ratio)}
              </p>
            </div>

            {/* 索提诺比率 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">索提诺比率</p>
              <p class="text-xl font-bold text-gray-900">
                {formatNumber(result()!.sortino_ratio)}
              </p>
            </div>

            {/* 胜率 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">胜率</p>
              <p class="text-xl font-bold text-gray-900">
                {formatNumber(result()!.win_rate)}%
              </p>
            </div>

            {/* 盈亏比 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">盈亏比</p>
              <p class="text-xl font-bold text-gray-900">
                {formatNumber(result()!.profit_factor)}
              </p>
            </div>

            {/* 总交易次数 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">总交易次数</p>
              <p class="text-xl font-bold text-gray-900">{result()!.total_trades}</p>
            </div>

            {/* 利润因子 */}
            <div class="bg-white rounded-xl border border-gray-200/60 p-4">
              <p class="text-[13px] font-medium text-gray-400 mb-1">利润因子</p>
              <p class="text-xl font-bold text-gray-900">
                {formatNumber(result()!.profit_factor)}
              </p>
            </div>
          </div>

          {/* 额外信息行 */}
          <div class="bg-white rounded-xl border border-gray-200/60 p-4">
            <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4 text-sm">
              <div>
                <span class="text-[13px] text-gray-400">盈利交易</span>
                <p class="font-semibold text-emerald-600 mt-0.5">{result()!.profit_trades}</p>
              </div>
              <div>
                <span class="text-[13px] text-gray-400">亏损交易</span>
                <p class="font-semibold text-red-500 mt-0.5">{result()!.loss_trades}</p>
              </div>
              <div>
                <span class="text-[13px] text-gray-400">平均盈利</span>
                <p class="font-semibold text-emerald-600 mt-0.5">
                  {formatNumber(result()!.avg_profit)}
                </p>
              </div>
              <div>
                <span class="text-[13px] text-gray-400">平均亏损</span>
                <p class="font-semibold text-red-500 mt-0.5">
                  {formatNumber(result()!.avg_loss)}
                </p>
              </div>
              <div>
                <span class="text-[13px] text-gray-400">最大连胜</span>
                <p class="font-semibold text-gray-800 mt-0.5">
                  {result()!.max_consecutive_wins}
                </p>
              </div>
              <div>
                <span class="text-[13px] text-gray-400">最大连亏</span>
                <p class="font-semibold text-gray-800 mt-0.5">
                  {result()!.max_consecutive_losses}
                </p>
              </div>
            </div>
          </div>

          {/* 交易信号 K 线图 */}
          <Show when={result()?.trades && result()!.trades.length > 0}>
            <div class="mb-6">
              <h4 class="text-[13px] font-semibold text-gray-500 mb-4">交易信号</h4>
              <KlineChart
                data={backtestKlineData()}
                height={300}
                markers={backtestMarkers()}
                overlays={backtestOverlays()}
              />
            </div>
          </Show>

          {/* 权益曲线图 */}
          <div class="bg-white rounded-xl border border-gray-200/60 p-5">
            <h4 class="text-[13px] font-semibold text-gray-500 mb-4">权益曲线</h4>
            <Show when={result()?.equity_curve && result()!.equity_curve.length > 0}>
              <EquityChart
                data={result()!.equity_curve}
                height={250}
                initialBalance={result()!.initial_balance}
              />
            </Show>
          </div>

          {/* 交易记录表格 */}
          <div class="bg-white rounded-xl border border-gray-200/60 p-5">
            <h4 class="text-[13px] font-semibold text-gray-500 mb-4">
              交易记录 ({result()!.trades.length} 笔)
            </h4>
            <div class="overflow-x-auto">
              <table class="w-full text-sm">
                <thead>
                  <tr class="border-b border-gray-100">
                    <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">入场时间</th>
                    <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">出场时间</th>
                    <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">方向</th>
                    <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">入场价</th>
                    <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">出场价</th>
                    <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">数量</th>
                    <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">盈亏</th>
                    <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">盈亏%</th>
                  </tr>
                </thead>
                <tbody>
                  <For each={result()!.trades}>
                    {(trade) => (
                      <tr class="border-b border-gray-50 hover:bg-gray-50/50 transition-colors">
                        <td class="py-2.5 px-3 text-gray-600 text-[13px]">{formatDateTime(trade.entry_time)}</td>
                        <td class="py-2.5 px-3 text-gray-600 text-[13px]">{formatDateTime(trade.exit_time)}</td>
                        <td class="py-2.5 px-3">
                          <span
                            class={`inline-block px-2 py-0.5 rounded-md text-xs font-medium ${
                              trade.side === 'buy'
                                ? 'bg-emerald-50 text-emerald-600'
                                : 'bg-red-50 text-red-500'
                            }`}
                          >
                            {trade.side === 'buy' ? '买入' : '卖出'}
                          </span>
                        </td>
                        <td class="py-2.5 px-3 text-right text-gray-600 text-[13px]">
                          {formatNumber(trade.entry_price)}
                        </td>
                        <td class="py-2.5 px-3 text-right text-gray-600 text-[13px]">
                          {formatNumber(trade.exit_price)}
                        </td>
                        <td class="py-2.5 px-3 text-right text-gray-600 text-[13px]">
                          {formatNumber(trade.quantity, 4)}
                        </td>
                        <td
                          class={`py-2.5 px-3 text-right font-medium text-[13px] ${
                            trade.pnl >= 0 ? 'text-emerald-600' : 'text-red-500'
                          }`}
                        >
                          {formatNumber(trade.pnl)}
                        </td>
                        <td
                          class={`py-2.5 px-3 text-right font-medium text-[13px] ${
                            trade.pnl_pct >= 0 ? 'text-emerald-600' : 'text-red-500'
                          }`}
                        >
                          {formatPct(trade.pnl_pct)}
                        </td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </Show>

      {/* 历史回测列表 */}
      <div class="bg-white rounded-xl border border-gray-200/60 p-6">
        <h3 class="text-[15px] font-semibold text-gray-800 mb-5">历史回测</h3>

        <Show
          when={!historyLoading()}
          fallback={
            <div class="text-center py-12">
              <div class="inline-block w-5 h-5 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin"></div>
              <p class="text-sm text-gray-400 mt-2">加载中...</p>
            </div>
          }
        >
          <Show
            when={historyError() === ''}
            fallback={
              <div class="text-center py-12">
                <p class="text-sm text-red-400">{historyError()}</p>
                <button
                  class="mt-2 text-sm text-indigo-600 hover:text-indigo-700"
                  onClick={() => loadHistory(historyPage())}
                >
                  重试
                </button>
              </div>
            }
          >
            <Show
              when={historyItems().length > 0}
              fallback={
                <div class="text-center py-12">
                  <svg class="w-12 h-12 mx-auto text-gray-200 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <p class="text-gray-400 text-sm">暂无历史回测记录</p>
                </div>
              }
            >
              <div class="overflow-x-auto">
                <table class="w-full text-sm">
                  <thead>
                    <tr class="border-b border-gray-100">
                      <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">策略名</th>
                      <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">交易对</th>
                      <th class="text-left py-2.5 px-3 text-[13px] font-medium text-gray-400">交易所</th>
                      <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">初始资金</th>
                      <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">最终资金</th>
                      <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">收益率</th>
                      <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">夏普比率</th>
                      <th class="text-right py-2.5 px-3 text-[13px] font-medium text-gray-400">最大回撤</th>
                      <th class="text-center py-2.5 px-3 text-[13px] font-medium text-gray-400">操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={historyItems()}>
                      {(item) => (
                        <tr class="border-b border-gray-50 hover:bg-gray-50/50 transition-colors">
                          <td class="py-2.5 px-3 text-gray-700">{item.strategy_name}</td>
                          <td class="py-2.5 px-3 text-gray-700">{item.symbol}</td>
                          <td class="py-2.5 px-3 text-gray-700">{item.exchange}</td>
                          <td class="py-2.5 px-3 text-right text-gray-600 text-[13px]">
                            {formatNumber(item.initial_balance)}
                          </td>
                          <td class="py-2.5 px-3 text-right text-gray-600 text-[13px]">
                            {formatNumber(item.final_balance)}
                          </td>
                          <td
                            class={`py-2.5 px-3 text-right font-medium text-[13px] ${
                              item.total_return_pct >= 0 ? 'text-emerald-600' : 'text-red-500'
                            }`}
                          >
                            {formatPct(item.total_return_pct)}
                          </td>
                          <td class="py-2.5 px-3 text-right text-gray-600 text-[13px]">
                            {formatNumber(item.sharpe_ratio)}
                          </td>
                          <td class="py-2.5 px-3 text-right text-red-500 text-[13px]">
                            {formatPct(-Math.abs(item.max_drawdown_pct))}
                          </td>
                          <td class="py-2.5 px-3 text-center">
                            <button
                              class="text-indigo-600 hover:text-indigo-700 text-[13px] font-medium"
                              onClick={() => handleViewDetail(item.id)}
                            >
                              查看详情
                            </button>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </div>

              {/* 分页 */}
              <Show when={historyTotalPages() > 1}>
                <div class="flex items-center justify-between mt-4 pt-4 border-t border-gray-100">
                  <p class="text-[13px] text-gray-400">
                    第 {historyPage()} / {historyTotalPages()} 页
                  </p>
                  <div class="flex gap-2">
                    <button
                      class="px-3 py-1.5 text-sm border border-gray-200 rounded-lg text-gray-600 hover:bg-gray-50 disabled:opacity-40 disabled:cursor-not-allowed"
                      disabled={historyPage() <= 1}
                      onClick={() => loadHistory(historyPage() - 1)}
                    >
                      上一页
                    </button>
                    <button
                      class="px-3 py-1.5 text-sm border border-gray-200 rounded-lg text-gray-600 hover:bg-gray-50 disabled:opacity-40 disabled:cursor-not-allowed"
                      disabled={historyPage() >= historyTotalPages()}
                      onClick={() => loadHistory(historyPage() + 1)}
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
    </div>
  )
}

export default Backtest
