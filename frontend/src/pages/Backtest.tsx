import { type Component, createSignal, createEffect, Show, For, onMount } from 'solid-js'
import { api, type PaginatedResponse } from '../lib/api'

// ---- 类型定义 ----

interface BacktestRequest {
  strategy_type: string
  symbol: string
  exchange: string
  timeframe: string
  start_date: string
  end_date: string
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

const DEFAULT_INDICATOR_CONFIGS: Record<string, Record<string, unknown>> = {
  sma_crossover: { short_period: 10, long_period: 30 },
  rsi: { period: 14, overbought: 70, oversold: 30 },
  macd: { fast_period: 12, slow_period: 26, signal_period: 9 },
  bollinger_bands: { period: 20, std_dev: 2 },
}

const DEFAULT_TRADING_CONFIG: Record<string, unknown> = {
  fixed_amount: 100,
  max_position_size: 1000,
}

// ---- 工具函数 ----

function formatNumber(n: number, decimals = 2): string {
  return n.toFixed(decimals)
}

function formatPct(n: number): string {
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
  // 表单状态
  const [strategyType, setStrategyType] = createSignal('sma_crossover')
  const [symbol, setSymbol] = createSignal('BTCUSDT')
  const [exchange, setExchange] = createSignal('binance')
  const [timeframe, setTimeframe] = createSignal('1h')
  const [startDate, setStartDate] = createSignal('')
  const [endDate, setEndDate] = createSignal('')
  const [initialBalance, setInitialBalance] = createSignal(10000)
  const [indicatorConfig, setIndicatorConfig] = createSignal(
    JSON.stringify(DEFAULT_INDICATOR_CONFIGS.sma_crossover, null, 2)
  )
  const [tradingConfig, setTradingConfig] = createSignal(
    JSON.stringify(DEFAULT_TRADING_CONFIG, null, 2)
  )

  // 运行状态
  const [running, setRunning] = createSignal(false)
  const [runError, setRunError] = createSignal('')

  // 结果状态
  const [result, setResult] = createSignal<BacktestResult | null>(null)

  // 历史列表状态
  const [historyItems, setHistoryItems] = createSignal<BacktestSummary[]>([])
  const [historyPage, setHistoryPage] = createSignal(1)
  const [historyTotalPages, setHistoryTotalPages] = createSignal(0)
  const [historyLoading, setHistoryLoading] = createSignal(false)
  const [historyError, setHistoryError] = createSignal('')

  // 策略类型变化时更新默认指标配置
  createEffect(() => {
    const st = strategyType()
    const defaultConfig = DEFAULT_INDICATOR_CONFIGS[st]
    if (defaultConfig) {
      setIndicatorConfig(JSON.stringify(defaultConfig, null, 2))
    }
  })

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

    const req: BacktestRequest = {
      strategy_type: strategyType(),
      symbol: symbol(),
      exchange: exchange(),
      timeframe: timeframe(),
      start_date: startDate(),
      end_date: endDate(),
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

  // 绘制权益曲线
  function drawEquityCurve(canvas: HTMLCanvasElement, data: [string, number][]) {
    if (!data || data.length < 2) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const dpr = window.devicePixelRatio || 1
    const rect = canvas.getBoundingClientRect()
    canvas.width = rect.width * dpr
    canvas.height = rect.height * dpr
    ctx.scale(dpr, dpr)

    const w = rect.width
    const h = rect.height
    const padding = { top: 20, right: 20, bottom: 30, left: 70 }
    const chartW = w - padding.left - padding.right
    const chartH = h - padding.top - padding.bottom

    const values = data.map((d) => d[1])
    const minVal = Math.min(...values)
    const maxVal = Math.max(...values)
    const range = maxVal - minVal || 1

    // 清空
    ctx.clearRect(0, 0, w, h)

    // 灰色网格
    ctx.strokeStyle = '#f3f4f6'
    ctx.lineWidth = 0.5
    const gridLines = 5
    for (let i = 0; i <= gridLines; i++) {
      const y = padding.top + (chartH / gridLines) * i
      ctx.beginPath()
      ctx.moveTo(padding.left, y)
      ctx.lineTo(w - padding.right, y)
      ctx.stroke()

      // Y 轴标签
      const val = maxVal - (range / gridLines) * i
      ctx.fillStyle = '#9ca3af'
      ctx.font = '11px sans-serif'
      ctx.textAlign = 'right'
      ctx.fillText(formatNumber(val), padding.left - 8, y + 4)
    }

    // X 轴标签 (取几个点)
    ctx.fillStyle = '#9ca3af'
    ctx.font = '11px sans-serif'
    ctx.textAlign = 'center'
    const labelCount = Math.min(6, data.length)
    const step = Math.floor(data.length / labelCount)
    for (let i = 0; i < data.length; i += step) {
      const x = padding.left + (i / (data.length - 1)) * chartW
      const dateStr = data[i][0]
      try {
        const d = new Date(dateStr)
        ctx.fillText(
          `${d.getMonth() + 1}/${d.getDate()}`,
          x,
          h - padding.bottom + 18
        )
      } catch {
        // ignore
      }
    }

    // 绿色折线
    ctx.strokeStyle = '#6366f1'
    ctx.lineWidth = 1.5
    ctx.beginPath()
    for (let i = 0; i < data.length; i++) {
      const x = padding.left + (i / (data.length - 1)) * chartW
      const y = padding.top + chartH - ((data[i][1] - minVal) / range) * chartH
      if (i === 0) {
        ctx.moveTo(x, y)
      } else {
        ctx.lineTo(x, y)
      }
    }
    ctx.stroke()

    // 填充区域
    const gradient = ctx.createLinearGradient(0, padding.top, 0, h - padding.bottom)
    gradient.addColorStop(0, 'rgba(99, 102, 241, 0.12)')
    gradient.addColorStop(1, 'rgba(99, 102, 241, 0.01)')
    ctx.lineTo(padding.left + chartW, h - padding.bottom)
    ctx.lineTo(padding.left, h - padding.bottom)
    ctx.closePath()
    ctx.fillStyle = gradient
    ctx.fill()
  }

  // 结果出现后绘制图表
  createEffect(() => {
    const r = result()
    if (r && r.equity_curve && r.equity_curve.length > 0) {
      // 使用 requestAnimationFrame 确保 canvas 已渲染
      requestAnimationFrame(() => {
        const canvas = document.getElementById('equity-canvas') as HTMLCanvasElement | null
        if (canvas) {
          drawEquityCurve(canvas, r.equity_curve)
        }
      })
    }
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
          {/* 策略类型 */}
          <div>
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">策略类型</label>
            <select
              class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 bg-white"
              value={strategyType()}
              onChange={(e) => setStrategyType(e.currentTarget.value)}
            >
              <option value="sma_crossover">SMA 均线交叉</option>
              <option value="rsi">RSI 相对强弱</option>
              <option value="macd">MACD</option>
              <option value="bollinger_bands">布林带</option>
            </select>
          </div>

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
          <div class="md:col-span-1 lg:col-span-1">
            <label class="block text-[13px] font-medium text-gray-400 mb-1.5">交易配置 (JSON)</label>
            <textarea
              class="w-full border border-gray-200 rounded-lg px-3 py-2 text-sm font-mono text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400"
              rows={4}
              value={tradingConfig()}
              onInput={(e) => setTradingConfig(e.currentTarget.value)}
            />
          </div>
        </div>

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

          {/* 权益曲线图 */}
          <div class="bg-white rounded-xl border border-gray-200/60 p-5">
            <h4 class="text-[13px] font-semibold text-gray-500 mb-4">权益曲线</h4>
            <canvas
              id="equity-canvas"
              class="w-full"
              style={{ height: '300px' }}
            />
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
