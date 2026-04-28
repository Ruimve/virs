import { type Component, createSignal, Show, For, onMount } from 'solid-js'
import { api, type PaginatedResponse } from '../../lib/api'

// ---- 类型定义 ----

interface GridLevel {
  level: number
  price: number
  side: 'buy' | 'sell'
  quantity_usdt: number
}

interface AiAnalysisResult {
  market_regime: string
  confidence: number
  recommended_action: string
  action_reason: string
  upper_price: number
  lower_price: number
  grid_count: number
  grid_profit_pct: number
  quantity_per_grid: number
  leverage: number
  grid_levels?: GridLevel[]
  analysis: string
  risk_warning: string
}

interface GridBot {
  id: string
  name: string
  symbol: string
  exchange: string
  status: 'draft' | 'running' | 'paused' | 'stopped' | 'error'
  upper_price: number
  lower_price: number
  grid_count: number
  grid_profit_pct: number
  quantity_per_grid: number
  leverage: number
  dynamic_adjust: boolean
  adjust_interval_secs: number
  total_pnl: number
  total_trades: number
  grid_filled_count: number
  market_regime: string
  ai_analysis: string
  created_at: string
  updated_at: string
}

interface GridTrade {
  id: string
  bot_id: string
  created_at: string
  side: 'buy' | 'sell'
  grid_level: number
  price: number
  quantity: number
  pnl: number
  pnl_pct: number
}

// ---- 默认 Prompt ----

const DEFAULT_SYSTEM_PROMPT = `你是一位专业的加密货币量化交易分析师，精通合约网格交易策略。你的职责是分析市场数据、判断市场状态、生成最优网格参数，并给出可执行的交易操作指令。

## 核心参数
- 交易对由用户提供
- 网格层数由用户提供（默认 50）
- 总投资额由用户提供（USDT）
- 杠杆倍数由用户提供
- 价格分布采用高斯分布（中间密、两端疏）

## 市场状态判断规则

### 震荡市场（适合网格交易）
- BBands Width < 3%（布林带收缩，价格在通道内运行）
- EMA20 与 EMA50 距离 < 1%（均线粘合，无明确方向）
- 价格在布林带中轨 ±1% 附近
- ADX < 25（趋势不明显）
- **操作**: 正常运行网格，place_buy_limit / place_sell_limit

### 趋势市场（暂停网格）
- BBands Width > 4%（布林带扩张，趋势启动）
- EMA20 与 EMA50 距离 > 2%（均线发散，方向明确）
- 价格持续突破布林带上轨或下轨（连续 3 根以上）
- ADX > 30（趋势强劲）
- **操作**: pause_grid，等待回归震荡后再 resume_grid

### 高波动市场（谨慎运行）
- ATR 异常放大（当前 ATR > 20 日 ATR 均值的 2 倍）
- 价格在短时间内剧烈波动（1h K 线实体 > ATR 的 1.5 倍）
- BBands Width 突然扩张（5 根 bar 内增幅 > 50%）
- **操作**: 可继续运行但减小仓位（quantity_per_grid × 0.5），或 pause_grid

## 网格参数计算规则

### 上下界确定
- 上界：近期阻力位（近期高点、BBands 上轨、整数关口）取最低值
- 下界：近期支撑位（近期低点、BBands 下轨、整数关口）取最高值
- 网格区间应覆盖当前价格 ±2 个标准差（约 95% 置信区间）
- 区间宽度 = 上界 - 下界，应 >= ATR × 10（确保足够的交易空间）

### 高斯分布网格
- 网格价格按高斯分布排列：中间密度高、两端密度低
- 使用当前价格为均值 μ，区间宽度 / 4 为标准差 σ
- 每个网格价格 = μ + σ × Φ⁻¹(p)，其中 p 按网格序号均匀分布
- 这样在价格密集区域（中间）有更多网格，捕捉更多交易机会

### 每格利润率
- 基础利润率 = (网格间距 / 网格价格) × 100%
- 考虑手续费（taker 0.05% × 2 = 0.1%），实际利润率应 > 0.3%
- 建议每格利润率 0.3% - 2.0%，波动率越高利润率可越大

### 每格数量
- 每格数量(USDT) = 总投资额 / 有效网格数
- 有效网格数 ≈ grid_count × 0.6（高斯分布下约 60% 的网格在 1σ 内）
- 实际下单数量 = 每格数量 / 杠杆倍数 / 当前价格（换算为币数）

## 可执行操作指令
- \`place_buy_limit\` — 在指定价格挂买单
- \`place_sell_limit\` — 在指定价格挂卖单
- \`cancel_order\` — 取消指定订单
- \`cancel_all_orders\` — 取消所有挂单
- \`pause_grid\` — 暂停网格（趋势市场时）
- \`resume_grid\` — 恢复网格（回归震荡时）
- \`adjust_grid\` — 调整网格上下界
- \`hold\` — 保持当前状态不操作

## 风控规则
1. 单次最大持仓不超过总投资的 30%
2. 网格区间内最大亏损不超过总投资的 15%
3. 当价格突破网格区间时，立即 cancel_all_orders 并 pause_grid
4. 当连续 3 次交易亏损时，减小仓位至 50%
5. 杠杆使用不超过 10 倍，高波动市场不超过 3 倍

## 输出格式（严格 JSON，不要 markdown 代码块）
{
  "market_regime": "ranging|trending_up|trending_down|volatile",
  "confidence": 0.0-1.0,
  "recommended_action": "run_grid|pause_grid|reduce_position|adjust_grid",
  "action_reason": "推荐操作的理由（50字以内）",
  "upper_price": 数字（网格上界）,
  "lower_price": 数字（网格下界）,
  "grid_count": 数字（网格层数）,
  "grid_profit_pct": 数字（每格利润率%）,
  "quantity_per_grid": 数字（每格数量，USDT）,
  "leverage": 数字（杠杆倍数）,
  "grid_levels": [
    { "level": 1, "price": 数字, "side": "buy", "quantity_usdt": 数字 },
    { "level": 2, "price": 数字, "side": "buy", "quantity_usdt": 数字 },
    ...
    { "level": N, "price": 数字, "side": "sell", "quantity_usdt": 数字 }
  ],
  "analysis": "详细分析说明（300字以内）",
  "risk_warning": "风险提示（100字以内）"
}`

const DEFAULT_USER_PROMPT = `## 当前时间：{current_time}
## 市场数据（除特别标注外均为 1h 周期）
- 当前价格：${'$'}{current_price}
- 1小时涨跌：{change_1h}
- 4小时涨跌：{change_4h}
- 24小时涨跌：{change_24h}
- ATR(14, 1h)：${'$'}{atr}（{atr_pct}）
- 布林带(1h)：上轨 ${'$'}{bb_upper}，中轨 ${'$'}{bb_middle}，下轨 ${'$'}{bb_lower}
- 布林带宽度(1h)：{bb_width}
- EMA(12, 1h)：{ema12}，方向：{ema12_trend}
- EMA(20, 1h)：{ema20}，方向：{ema20_trend}
- EMA(26, 1h)：{ema26}，方向：{ema26_trend}
- EMA(50, 1h)：{ema50}，方向：{ema50_trend}
- MACD(1h)：{macd}，Signal：{macd_signal}，Histogram：{macd_histogram}
- ADX(14, 1h)：{adx}
- EMA(26, 4h)：{ema_4h}
- 资金费率：{funding_rate}%
- 24h 波动率：{volatility}%
- RSI(14, 1h)：{rsi}

## 账户状态
- 总权益：（待接入）
- 可用余额：（待接入）
- 当前持仓：（待接入）
- 未实现盈亏：（待接入）

## 网格状态
- 网格范围：（待创建后显示）
- 网格间距：（待创建后显示）
- 活跃订单数：（待运行后显示）
- 已成交层数：（待运行后显示）
- 网格已暂停：false

请根据以上市场数据，判断当前市场状态，生成最优网格交易参数。`

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
    return new Date(s).toLocaleString('zh-CN', {
      month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit',
    })
  } catch { return s }
}

const marketRegimeLabel: Record<string, string> = {
  ranging: '震荡',
  trending_up: '上升趋势',
  trending_down: '下降趋势',
  volatile: '高波动',
}

const marketRegimeColor: Record<string, string> = {
  ranging: 'bg-slate-100 text-slate-600',
  trending_up: 'bg-emerald-50 text-emerald-600',
  trending_down: 'bg-red-50 text-red-500',
  volatile: 'bg-amber-50 text-amber-600',
}

// ---- 组件 ----

const GridBotPage: Component = () => {
  // ── 创建表单 ──
  const [symbol, setSymbol] = createSignal('BTCUSDT')
  const [exchange, setExchange] = createSignal('binance')
  const [systemPrompt, setSystemPrompt] = createSignal(DEFAULT_SYSTEM_PROMPT)
  const [userPrompt, setUserPrompt] = createSignal(DEFAULT_USER_PROMPT)

  // AI 分析
  const [aiLoading, setAiLoading] = createSignal(false)
  const [aiResult, setAiResult] = createSignal<AiAnalysisResult | null>(null)
  const [finalUserPrompt, setFinalUserPrompt] = createSignal<string | null>(null)
  const [aiError, setAiError] = createSignal('')

  // 网格参数（可被 AI 填充，也可手动输入）
  const [upperPrice, setUpperPrice] = createSignal<number | null>(null)
  const [lowerPrice, setLowerPrice] = createSignal<number | null>(null)
  const [gridCount, setGridCount] = createSignal<number | null>(null)
  const [gridProfitPct, setGridProfitPct] = createSignal<number | null>(null)
  const [quantityPerGrid, setQuantityPerGrid] = createSignal<number | null>(null)
  const [leverage, setLeverage] = createSignal<number | null>(null)

  // 机器人名称 & 动态调整
  const [botName, setBotName] = createSignal('')
  const [dynamicAdjust, setDynamicAdjust] = createSignal(true)
  const [adjustInterval, setAdjustInterval] = createSignal(300)

  // 创建
  const [creating, setCreating] = createSignal(false)
  const [createError, setCreateError] = createSignal('')
  const [createSuccess, setCreateSuccess] = createSignal('')

  // ── 机器人列表 ──
  const [bots, setBots] = createSignal<GridBot[]>([])
  const [botsLoading, setBotsLoading] = createSignal(false)
  const [botsError, setBotsError] = createSignal('')
  const [botsPage, setBotsPage] = createSignal(1)
  const [botsTotalPages, setBotsTotalPages] = createSignal(0)

  // ── 交易记录弹窗 ──
  const [tradesBotId, setTradesBotId] = createSignal<string | null>(null)
  const [tradesBotName, setTradesBotName] = createSignal('')
  const [trades, setTrades] = createSignal<GridTrade[]>([])
  const [tradesLoading, setTradesLoading] = createSignal(false)
  const [tradesError, setTradesError] = createSignal('')
  const [tradesPage, setTradesPage] = createSignal(1)
  const [tradesTotalPages, setTradesTotalPages] = createSignal(0)

  // ── 操作状态 ──
  const [actionLoading, setActionLoading] = createSignal<string | null>(null)

  // ── AI 分析 ──

  async function handleAiAnalyze() {
    setAiLoading(true)
    setAiError('')
    setAiResult(null)
    try {
      const sysPrompt = systemPrompt()
      const usrPrompt = userPrompt()
        .replace('{symbol}', symbol())
        .replace('{exchange}', exchange())
      const res = await api.post<{ analysis: AiAnalysisResult; user_prompt?: string; system_prompt?: string }>('/grid/analyze', {
        symbol: symbol(),
        exchange: exchange(),
        system_prompt: sysPrompt,
        user_prompt: usrPrompt,
      })
      if (res.success && res.data) {
        const analysis: AiAnalysisResult = res.data.analysis
        setAiResult(analysis)
        if (res.data.user_prompt) setFinalUserPrompt(res.data.user_prompt)
        if (res.data.system_prompt) setSystemPrompt(res.data.system_prompt)
        setUpperPrice(analysis.upper_price)
        setLowerPrice(analysis.lower_price)
        setGridCount(analysis.grid_count)
        setGridProfitPct(analysis.grid_profit_pct)
        setQuantityPerGrid(analysis.quantity_per_grid)
        setLeverage(analysis.leverage)
        // 自动生成名称
        if (!botName()) {
          setBotName(`网格机器人-${symbol()}`)
        }
      } else {
        setAiError(res.error || 'AI 分析失败')
      }
    } catch (e) {
      setAiError(e instanceof Error ? e.message : 'AI 分析失败')
    } finally {
      setAiLoading(false)
    }
  }

  // ── 创建机器人 ──

  async function handleCreate() {
    setCreating(true)
    setCreateError('')
    setCreateSuccess('')
    try {
      const name = botName().trim()
      if (!name) { setCreateError('请输入机器人名称'); setCreating(false); return }
      if (!upperPrice() || !lowerPrice()) { setCreateError('请设置网格上下界'); setCreating(false); return }
      if (upperPrice()! <= lowerPrice()!) { setCreateError('上界必须大于下界'); setCreating(false); return }
      if (!gridCount() || gridCount()! < 2) { setCreateError('网格数量至少为 2'); setCreating(false); return }

      const res = await api.post('/grid/create', {
        name,
        symbol: symbol(),
        exchange: exchange(),
        upper_price: upperPrice(),
        lower_price: lowerPrice(),
        grid_count: gridCount(),
        grid_profit_pct: gridProfitPct(),
        quantity_per_grid: quantityPerGrid(),
        leverage: leverage(),
        dynamic_adjust: dynamicAdjust(),
        adjust_interval_secs: adjustInterval(),
        system_prompt: systemPrompt(),
        user_prompt: userPrompt(),
        market_regime: aiResult()?.market_regime,
        ai_analysis: aiResult()?.analysis,
      })
      if (res.success) {
        setCreateSuccess('机器人创建成功并已启动')
        // 重置表单
        setAiResult(null)
        setUpperPrice(null)
        setLowerPrice(null)
        setGridCount(null)
        setGridProfitPct(null)
        setQuantityPerGrid(null)
        setLeverage(null)
        setBotName('')
        // 刷新列表
        loadBots(1)
      } else {
        setCreateError(res.error || '创建失败')
      }
    } catch (e) {
      setCreateError(e instanceof Error ? e.message : '创建失败')
    } finally {
      setCreating(false)
    }
  }

  // ── 加载机器人列表 ──

  async function loadBots(page: number) {
    setBotsLoading(true)
    setBotsError('')
    try {
      const res = await api.get<PaginatedResponse<GridBot>>(`/grid/list?page=${page}&page_size=10`)
      if (res.success && res.data) {
        setBots(res.data.items)
        setBotsTotalPages(res.data.total_pages)
        setBotsPage(res.data.page)
      } else {
        setBotsError(res.error || '加载失败')
      }
    } catch (e) {
      setBotsError(e instanceof Error ? e.message : '加载失败')
    } finally {
      setBotsLoading(false)
    }
  }

  // ── 启动/停止 ──

  async function handleToggleBot(id: string, action: 'start' | 'stop') {
    setActionLoading(id)
    try {
      const res = await api.post(`/grid/${id}/${action}`)
      if (res.success) {
        loadBots(botsPage())
      }
    } catch { /* */ }
    finally {
      setActionLoading(null)
    }
  }

  // ── 重新分析 ──

  async function handleReanalyze(id: string) {
    setActionLoading(id)
    try {
      const res = await api.post(`/grid/${id}/reanalyze`)
      if (res.success) {
        loadBots(botsPage())
      }
    } catch { /* */ }
    finally {
      setActionLoading(null)
    }
  }

  // ── 删除 ──

  async function handleDelete(id: string) {
    if (!confirm('确定要删除此机器人吗？')) return
    setActionLoading(id)
    try {
      const res = await api.del(`/grid/${id}/delete`)
      if (res.success) {
        loadBots(botsPage())
      }
    } catch { /* */ }
    finally {
      setActionLoading(null)
    }
  }

  // ── 交易记录 ──

  async function loadTrades(botId: string, page: number) {
    setTradesLoading(true)
    setTradesError('')
    try {
      const res = await api.get<PaginatedResponse<GridTrade>>(`/grid/${botId}/trades?page=${page}&page_size=20`)
      if (res.success && res.data) {
        setTrades(res.data.items)
        setTradesTotalPages(res.data.total_pages)
        setTradesPage(res.data.page)
      } else {
        setTradesError(res.error || '加载失败')
      }
    } catch (e) {
      setTradesError(e instanceof Error ? e.message : '加载失败')
    } finally {
      setTradesLoading(false)
    }
  }

  function openTrades(bot: GridBot) {
    setTradesBotId(bot.id)
    setTradesBotName(bot.name)
    setTrades([])
    setTradesPage(1)
    setTradesTotalPages(0)
    loadTrades(bot.id, 1)
  }

  function closeTrades() {
    setTradesBotId(null)
    setTradesBotName('')
    setTrades([])
  }

  // ── 初始化 ──

  onMount(() => loadBots(1))

  // ── 样式 ──

  const inp = "w-full bg-slate-50 border border-slate-200 rounded-lg px-3 py-[7px] text-[13px] text-slate-800 placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-300 focus:bg-white transition-all"
  const sel = "w-full bg-slate-50 border border-slate-200 rounded-lg px-3 py-[7px] text-[13px] text-slate-800 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-300 focus:bg-white transition-all appearance-none cursor-pointer"

  return (
    <div class="space-y-4">
      {/* ═══ 创建网格机器人 ═══ */}
      <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
        {/* 标题 */}
        <div class="px-5 py-4 border-b border-slate-100">
          <div class="flex items-center gap-3">
            <div class="w-1 h-5 bg-indigo-500 rounded-full" />
            <h3 class="text-[14px] font-semibold text-slate-800">创建网格机器人</h3>
          </div>
        </div>

        <div class="p-5 space-y-4">
          {/* 交易对 & 交易所 */}
          <div class="flex flex-wrap items-end gap-x-3 gap-y-2.5">
            <div class="w-[150px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px] tracking-wide uppercase">交易对</label>
              <input type="text" class={inp} value={symbol()} onInput={e => setSymbol(e.currentTarget.value)} placeholder="BTCUSDT" />
            </div>
            <div class="w-[130px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px] tracking-wide uppercase">交易所</label>
              <select class={sel} value={exchange()} onChange={e => setExchange(e.currentTarget.value)}>
                <option value="binance">Binance</option>
                <option value="okx">OKX</option>
                <option value="bybit">Bybit</option>
              </select>
            </div>
          </div>

          {/* AI 分析区域 */}
          <div class="bg-slate-50/80 rounded-xl border border-slate-200/60 p-4 space-y-3">
            <div class="flex items-center gap-2 mb-1">
              <svg class="w-4 h-4 text-violet-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 002.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.455 2.456L21.75 6l-1.036.259a3.375 3.375 0 00-2.455 2.456z" />
              </svg>
              <span class="text-[13px] font-semibold text-slate-700">AI 分析</span>
            </div>

            <div>
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">System Prompt</label>
              <textarea
                class="w-full bg-white border border-slate-200 rounded-lg px-3 py-2 text-[13px] font-mono text-slate-800 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-300 transition-all resize-y"
                rows={8}
                value={systemPrompt()}
                onInput={e => setSystemPrompt(e.currentTarget.value)}
              />
            </div>

            <div>
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">User Prompt</label>
              <textarea
                class="w-full bg-white border border-slate-200 rounded-lg px-3 py-2 text-[13px] font-mono text-slate-800 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-300 transition-all resize-y"
                rows={3}
                value={userPrompt()}
                onInput={e => setUserPrompt(e.currentTarget.value)}
              />
            </div>

            <button
              class="px-4 py-[7px] bg-gradient-to-r from-violet-500 to-indigo-500 text-white text-[13px] font-semibold rounded-lg hover:from-violet-600 hover:to-indigo-600 active:from-violet-700 active:to-indigo-700 transition-all disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap shadow-sm shadow-violet-200/50"
              disabled={aiLoading()}
              onClick={handleAiAnalyze}
            >
              {aiLoading() ? (
                <span class="flex items-center gap-2">
                  <span class="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  分析中...
                </span>
              ) : (
                '✨ AI 分析市场'
              )}
            </button>
          </div>

          {/* AI 分析结果 */}
          <Show when={aiError()}>
            <div class="p-3 bg-red-50 border border-red-200/60 rounded-lg text-[13px] text-red-600 flex items-center gap-2">
              <svg class="w-4 h-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" />
              </svg>
              {aiError()}
            </div>
          </Show>

          <Show when={aiResult()}>
            <div class="bg-gradient-to-br from-violet-50/80 via-indigo-50/50 to-slate-50 rounded-xl border border-violet-100/60 p-4 space-y-3">
              <div class="flex items-center gap-3">
                <div class="w-1 h-5 bg-violet-500 rounded-full" />
                <h4 class="text-[13px] font-semibold text-slate-800">AI 分析结果</h4>
                <span class={`px-2 py-0.5 rounded-md text-[11px] font-semibold ${marketRegimeColor[aiResult()!.market_regime] || 'bg-slate-100 text-slate-500'}`}>
                  {marketRegimeLabel[aiResult()!.market_regime] || aiResult()!.market_regime}
                </span>
                <Show when={aiResult()!.confidence}>
                  <span class="text-[11px] text-slate-400">置信度 {Math.round((aiResult()!.confidence ?? 0) * 100)}%</span>
                </Show>
                <Show when={aiResult()!.recommended_action}>
                  <span class={`px-2 py-0.5 rounded-md text-[11px] font-semibold ${
                    aiResult()!.recommended_action === 'run_grid' ? 'bg-emerald-50 text-emerald-600' :
                    aiResult()!.recommended_action === 'pause_grid' ? 'bg-red-50 text-red-500' :
                    aiResult()!.recommended_action === 'reduce_position' ? 'bg-amber-50 text-amber-600' :
                    'bg-blue-50 text-blue-600'
                  }`}>
                    {aiResult()!.recommended_action === 'run_grid' ? '▶ 运行网格' :
                     aiResult()!.recommended_action === 'pause_grid' ? '⏸ 暂停网格' :
                     aiResult()!.recommended_action === 'reduce_position' ? '⚠ 减仓' :
                     aiResult()!.recommended_action === 'adjust_grid' ? '🔧 调整网格' :
                     aiResult()!.recommended_action}
                  </span>
                </Show>
              </div>

              <p class="text-[13px] text-slate-600 leading-relaxed">{aiResult()!.analysis}</p>

              {/* 网格参数 */}
              <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
                <div>
                  <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">网格上界</label>
                  <input type="number" class={inp} value={String(upperPrice() ?? '')} onInput={e => setUpperPrice(parseFloat(e.currentTarget.value) || null)} />
                </div>
                <div>
                  <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">网格下界</label>
                  <input type="number" class={inp} value={String(lowerPrice() ?? '')} onInput={e => setLowerPrice(parseFloat(e.currentTarget.value) || null)} />
                </div>
                <div>
                  <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">网格数量</label>
                  <input type="number" class={inp} value={String(gridCount() ?? '')} onInput={e => setGridCount(parseInt(e.currentTarget.value) || null)} min="2" max="50" />
                </div>
                <div>
                  <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">每格利润 %</label>
                  <input type="number" class={inp} value={String(gridProfitPct() ?? '')} onInput={e => setGridProfitPct(parseFloat(e.currentTarget.value) || null)} step="0.1" min="0.1" max="5" />
                </div>
                <div>
                  <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">每格数量 (USDT)</label>
                  <input type="number" class={inp} value={String(quantityPerGrid() ?? '')} onInput={e => setQuantityPerGrid(parseFloat(e.currentTarget.value) || null)} min="1" />
                </div>
                <div>
                  <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">杠杆</label>
                  <input type="number" class={inp} value={String(leverage() ?? '')} onInput={e => setLeverage(parseInt(e.currentTarget.value) || null)} min="1" max="10" />
                </div>
              </div>

              {/* 风险提示 */}
              <Show when={aiResult()!.risk_warning}>
                <div class="flex items-start gap-2 p-3 bg-amber-50/80 border border-amber-200/60 rounded-lg">
                  <svg class="w-4 h-4 text-amber-500 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" />
                  </svg>
                  <p class="text-[12px] text-amber-700 leading-relaxed">{aiResult()!.risk_warning}</p>
                </div>
              </Show>

              {/* 网格层级预览 */}
              <Show when={aiResult()!.grid_levels && aiResult()!.grid_levels!.length > 0}>
                <details class="mt-2">
                  <summary class="text-[12px] text-slate-400 cursor-pointer hover:text-slate-600 transition-colors select-none">
                    📊 查看网格层级（{aiResult()!.grid_levels!.length} 层）
                  </summary>
                  <div class="mt-2 max-h-[200px] overflow-auto border border-slate-200 rounded-lg">
                    <table class="w-full text-[11px]">
                      <thead class="sticky top-0 bg-slate-50">
                        <tr class="border-b border-slate-200">
                          <th class="py-1.5 px-2 text-left font-semibold text-slate-500">#</th>
                          <th class="py-1.5 px-2 text-right font-semibold text-slate-500">价格</th>
                          <th class="py-1.5 px-2 text-center font-semibold text-slate-500">方向</th>
                          <th class="py-1.5 px-2 text-right font-semibold text-slate-500">数量(USDT)</th>
                        </tr>
                      </thead>
                      <tbody>
                        <For each={aiResult()!.grid_levels!}>
                          {(gl) => (
                            <tr class="border-b border-slate-100 hover:bg-slate-50/50">
                              <td class="py-1 px-2 text-slate-500">{gl.level}</td>
                              <td class="py-1 px-2 text-right font-mono text-slate-700">{gl.price.toFixed(2)}</td>
                              <td class="py-1 px-2 text-center">
                                <span class={`px-1.5 py-[1px] rounded text-[10px] font-semibold ${gl.side === 'buy' ? 'bg-emerald-50 text-emerald-600' : 'bg-red-50 text-red-500'}`}>
                                  {gl.side === 'buy' ? '买入' : '卖出'}
                                </span>
                              </td>
                              <td class="py-1 px-2 text-right font-mono text-slate-600">{gl.quantity_usdt.toFixed(2)}</td>
                            </tr>
                          )}
                        </For>
                      </tbody>
                    </table>
                  </div>
                </details>
              </Show>

              {/* 最终 User Prompt */}
              <Show when={finalUserPrompt()}>
                <details class="mt-2">
                  <summary class="text-[12px] text-slate-400 cursor-pointer hover:text-slate-600 transition-colors select-none">
                    📋 查看 AI 实际接收的 User Prompt
                  </summary>
                  <pre class="mt-2 p-3 bg-slate-50 border border-slate-200 rounded-lg text-[11px] text-slate-600 leading-relaxed whitespace-pre-wrap overflow-auto max-h-[300px] font-mono">{finalUserPrompt()}</pre>
                </details>
              </Show>
            </div>
          </Show>

          {/* 手动参数（无 AI 结果时显示） */}
          <Show when={!aiResult()}>
            <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">网格上界</label>
                <input type="number" class={inp} value={String(upperPrice() ?? '')} onInput={e => setUpperPrice(parseFloat(e.currentTarget.value) || null)} placeholder="68000" />
              </div>
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">网格下界</label>
                <input type="number" class={inp} value={String(lowerPrice() ?? '')} onInput={e => setLowerPrice(parseFloat(e.currentTarget.value) || null)} placeholder="65000" />
              </div>
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">网格数量</label>
                <input type="number" class={inp} value={String(gridCount() ?? '')} onInput={e => setGridCount(parseInt(e.currentTarget.value) || null)} placeholder="20" min="2" max="50" />
              </div>
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">每格利润 %</label>
                <input type="number" class={inp} value={String(gridProfitPct() ?? '')} onInput={e => setGridProfitPct(parseFloat(e.currentTarget.value) || null)} placeholder="0.5" step="0.1" />
              </div>
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">每格数量 (USDT)</label>
                <input type="number" class={inp} value={String(quantityPerGrid() ?? '')} onInput={e => setQuantityPerGrid(parseFloat(e.currentTarget.value) || null)} placeholder="100" />
              </div>
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">杠杆</label>
                <input type="number" class={inp} value={String(leverage() ?? '')} onInput={e => setLeverage(parseInt(e.currentTarget.value) || null)} placeholder="5" min="1" max="10" />
              </div>
            </div>
          </Show>

          {/* 机器人名称 & 动态调整 */}
          <div class="flex flex-wrap items-end gap-x-3 gap-y-2.5">
            <div class="w-[260px]">
              <label class="block text-[11px] font-medium text-slate-400 mb-[3px] tracking-wide uppercase">机器人名称</label>
              <input type="text" class={inp} value={botName()} onInput={e => setBotName(e.currentTarget.value)} placeholder="网格机器人-BTCUSDT" />
            </div>
            <div class="flex items-center gap-2 py-[7px]">
              <label class="flex items-center gap-2 cursor-pointer select-none">
                <input
                  type="checkbox"
                  class="w-4 h-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-500/30"
                  checked={dynamicAdjust()}
                  onChange={e => setDynamicAdjust(e.currentTarget.checked)}
                />
                <span class="text-[13px] text-slate-700 font-medium">动态调整</span>
              </label>
            </div>
            <Show when={dynamicAdjust()}>
              <div class="w-[160px]">
                <label class="block text-[11px] font-medium text-slate-400 mb-[3px]">调整间隔 (秒)</label>
                <input type="number" class={inp} value={String(adjustInterval())} onInput={e => setAdjustInterval(parseInt(e.currentTarget.value) || 300)} min="60" step="60" />
              </div>
            </Show>
            <div class="ml-auto">
              <button
                class="px-6 py-[7px] bg-indigo-600 text-white text-[13px] font-semibold rounded-lg hover:bg-indigo-700 active:bg-indigo-800 transition-all duration-150 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap shadow-sm shadow-indigo-200"
                disabled={creating()}
                onClick={handleCreate}
              >
                {creating() ? (
                  <span class="flex items-center gap-2">
                    <span class="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                    创建中...
                  </span>
                ) : '创建并启动'}
              </button>
            </div>
          </div>

          {/* 创建成功/错误 */}
          <Show when={createSuccess()}>
            <div class="p-3 bg-emerald-50 border border-emerald-200/60 rounded-lg text-[13px] text-emerald-600 flex items-center gap-2">
              <svg class="w-4 h-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
              </svg>
              {createSuccess()}
            </div>
          </Show>
          <Show when={createError()}>
            <div class="p-3 bg-red-50 border border-red-200/60 rounded-lg text-[13px] text-red-600 flex items-center gap-2">
              <svg class="w-4 h-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" />
              </svg>
              {createError()}
            </div>
          </Show>
        </div>
      </div>

      {/* ═══ 我的网格机器人 ═══ */}
      <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
        <div class="px-5 py-4 border-b border-slate-100">
          <div class="flex items-center gap-3">
            <div class="w-1 h-5 bg-indigo-500 rounded-full" />
            <h3 class="text-[14px] font-semibold text-slate-800">我的网格机器人</h3>
          </div>
        </div>

        <Show when={!botsLoading()} fallback={
          <div class="flex items-center justify-center py-16">
            <div class="w-5 h-5 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" />
          </div>
        }>
          <Show when={botsError() === ''} fallback={
            <div class="text-center py-16">
              <p class="text-[13px] text-red-400">{botsError()}</p>
              <button class="mt-2 text-[13px] text-indigo-600 hover:text-indigo-700 font-medium" onClick={() => loadBots(botsPage())}>重试</button>
            </div>
          }>
            <Show when={bots().length > 0} fallback={
              <div class="text-center py-16">
                <p class="text-slate-400 text-[13px]">暂无网格机器人，请先创建一个</p>
              </div>
            }>
              <div class="divide-y divide-slate-100">
                <For each={bots()}>
                  {(bot) => (
                    <div class="px-5 py-4 hover:bg-slate-50/50 transition-colors">
                      <div class="flex flex-wrap items-start justify-between gap-3">
                        {/* 左侧信息 */}
                        <div class="flex-1 min-w-0">
                          <div class="flex items-center gap-2.5 mb-1.5">
                            <span class="text-[14px] font-semibold text-slate-800 truncate">{bot.name}</span>
                            <span class={`px-2 py-0.5 rounded-md text-[11px] font-semibold ${
                              bot.status === 'running' ? 'bg-emerald-50 text-emerald-600' :
                              bot.status === 'stopped' ? 'bg-slate-100 text-slate-500' :
                              'bg-red-50 text-red-500'
                            }`}>
                              {bot.status === 'running' ? '运行中' : bot.status === 'stopped' ? '已停止' : '错误'}
                            </span>
                            <span class={`px-2 py-0.5 rounded-md text-[11px] font-medium ${marketRegimeColor[bot.market_regime] || 'bg-slate-100 text-slate-500'}`}>
                              {marketRegimeLabel[bot.market_regime] || bot.market_regime}
                            </span>
                          </div>
                          <p class="text-[12px] text-slate-500 mb-1.5">
                            {bot.exchange} &middot; {bot.grid_count}格 &middot; {fmt(bot.grid_profit_pct)}%利润 &middot; {bot.leverage}x杠杆
                          </p>
                          <p class="text-[12px] text-slate-500 mb-1.5">
                            {fmt(bot.lower_price)} - {fmt(bot.upper_price)}
                            <span class="mx-2 text-slate-300">|</span>
                            PnL: <span class={`font-semibold ${bot.total_pnl >= 0 ? 'text-emerald-600' : 'text-red-500'}`}>
                              {bot.total_pnl >= 0 ? '+' : ''}{fmt(bot.total_pnl)}
                            </span>
                          </p>
                          <p class="text-[12px] text-slate-400">
                            交易次数: {bot.total_trades} &middot; 已填充网格: {bot.grid_filled_count}/{bot.grid_count}
                          </p>
                        </div>

                        {/* 右侧操作 */}
                        <div class="flex items-center gap-2 shrink-0">
                          {bot.status === 'running' ? (
                            <button
                              class="px-3 py-1.5 text-[12px] font-medium border border-slate-200 rounded-lg text-slate-600 hover:bg-slate-50 hover:text-slate-700 transition-colors disabled:opacity-50"
                              disabled={actionLoading() === bot.id}
                              onClick={() => handleToggleBot(bot.id, 'stop')}
                            >
                              停止
                            </button>
                          ) : (
                            <button
                              class="px-3 py-1.5 text-[12px] font-medium bg-emerald-50 text-emerald-600 rounded-lg hover:bg-emerald-100 transition-colors disabled:opacity-50"
                              disabled={actionLoading() === bot.id}
                              onClick={() => handleToggleBot(bot.id, 'start')}
                            >
                              启动
                            </button>
                          )}
                          <button
                            class="px-3 py-1.5 text-[12px] font-medium bg-violet-50 text-violet-600 rounded-lg hover:bg-violet-100 transition-colors disabled:opacity-50"
                            disabled={actionLoading() === bot.id}
                            onClick={() => handleReanalyze(bot.id)}
                          >
                            重新分析
                          </button>
                          <button
                            class="px-3 py-1.5 text-[12px] font-medium border border-slate-200 rounded-lg text-slate-600 hover:bg-slate-50 hover:text-slate-700 transition-colors"
                            onClick={() => openTrades(bot)}
                          >
                            查看交易
                          </button>
                          <button
                            class="px-3 py-1.5 text-[12px] font-medium text-red-500 hover:bg-red-50 rounded-lg transition-colors disabled:opacity-50"
                            disabled={actionLoading() === bot.id}
                            onClick={() => handleDelete(bot.id)}
                          >
                            删除
                          </button>
                        </div>
                      </div>
                    </div>
                  )}
                </For>
              </div>

              {/* 分页 */}
              <Show when={botsTotalPages() > 1}>
                <div class="flex items-center justify-between px-5 py-3 border-t border-slate-100">
                  <p class="text-[12px] text-slate-400">{botsPage()} / {botsTotalPages()}</p>
                  <div class="flex gap-2">
                    <button
                      class="px-3 py-1 text-[12px] border border-slate-200 rounded-lg text-slate-500 hover:bg-slate-50 hover:text-slate-700 disabled:opacity-30 transition-colors font-medium"
                      disabled={botsPage() <= 1}
                      onClick={() => loadBots(botsPage() - 1)}
                    >
                      上一页
                    </button>
                    <button
                      class="px-3 py-1 text-[12px] border border-slate-200 rounded-lg text-slate-500 hover:bg-slate-50 hover:text-slate-700 disabled:opacity-30 transition-colors font-medium"
                      disabled={botsPage() >= botsTotalPages()}
                      onClick={() => loadBots(botsPage() + 1)}
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

      {/* ═══ 交易记录弹窗 ═══ */}
      <Show when={tradesBotId() !== null}>
        <div class="fixed inset-0 z-50 flex items-center justify-center">
          {/* 遮罩 */}
          <div class="absolute inset-0 bg-black/30 backdrop-blur-sm" onClick={closeTrades} />

          {/* 弹窗 */}
          <div class="relative bg-white rounded-2xl border border-slate-200/80 shadow-xl w-full max-w-4xl max-h-[80vh] mx-4 flex flex-col overflow-hidden">
            {/* 标题 */}
            <div class="flex items-center justify-between px-5 py-4 border-b border-slate-100 shrink-0">
              <div class="flex items-center gap-3">
                <div class="w-1 h-5 bg-indigo-500 rounded-full" />
                <h3 class="text-[14px] font-semibold text-slate-800">交易记录</h3>
                <span class="text-[12px] text-slate-400">{tradesBotName()}</span>
              </div>
              <button
                class="p-1.5 rounded-lg text-slate-400 hover:bg-slate-100 hover:text-slate-600 transition-colors"
                onClick={closeTrades}
              >
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            {/* 内容 */}
            <div class="flex-1 overflow-auto">
              <Show when={!tradesLoading()} fallback={
                <div class="flex items-center justify-center py-16">
                  <div class="w-5 h-5 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" />
                </div>
              }>
                <Show when={tradesError() === ''} fallback={
                  <div class="text-center py-16">
                    <p class="text-[13px] text-red-400">{tradesError()}</p>
                  </div>
                }>
                  <Show when={trades().length > 0} fallback={
                    <div class="text-center py-16">
                      <p class="text-slate-400 text-[13px]">暂无交易记录</p>
                    </div>
                  }>
                    <table class="w-full text-[13px]">
                      <thead class="sticky top-0 bg-slate-50/95 backdrop-blur-sm">
                        <tr class="border-b border-slate-100">
                          <th class="text-left py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">时间</th>
                          <th class="text-left py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">方向</th>
                          <th class="text-right py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">网格级别</th>
                          <th class="text-right py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">价格</th>
                          <th class="text-right py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">数量</th>
                          <th class="text-right py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">盈亏</th>
                          <th class="text-right py-2.5 px-4 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">盈亏%</th>
                        </tr>
                      </thead>
                      <tbody>
                        <For each={trades()}>
                          {(trade, i) => (
                            <tr class={`border-b border-slate-50 hover:bg-slate-50/50 transition-colors ${i() % 2 === 0 ? '' : 'bg-slate-50/30'}`}>
                              <td class="py-2 px-4 text-slate-500 text-[12px] font-mono">{fmtDT(trade.created_at)}</td>
                              <td class="py-2 px-4">
                                <span class={`inline-flex items-center px-2 py-[2px] rounded-md text-[11px] font-semibold ${
                                  trade.side === 'buy' ? 'bg-emerald-50 text-emerald-600' : 'bg-red-50 text-red-500'
                                }`}>
                                  {trade.side === 'buy' ? '买' : '卖'}
                                </span>
                              </td>
                              <td class="py-2 px-4 text-right text-slate-500 font-mono text-[12px]">{trade.grid_level}</td>
                              <td class="py-2 px-4 text-right text-slate-600 font-mono text-[12px]">{fmt(trade.price)}</td>
                              <td class="py-2 px-4 text-right text-slate-500 font-mono text-[12px]">{fmt(trade.quantity, 4)}</td>
                              <td class={`py-2 px-4 text-right font-semibold text-[12px] font-mono ${trade.pnl >= 0 ? 'text-emerald-600' : 'text-red-500'}`}>
                                {trade.pnl >= 0 ? '+' : ''}{fmt(trade.pnl)}
                              </td>
                              <td class={`py-2 px-4 text-right font-semibold text-[12px] font-mono ${trade.pnl_pct >= 0 ? 'text-emerald-600' : 'text-red-500'}`}>
                                {fmtPct(trade.pnl_pct)}
                              </td>
                            </tr>
                          )}
                        </For>
                      </tbody>
                    </table>
                  </Show>
                </Show>
              </Show>
            </div>

            {/* 分页 */}
            <Show when={tradesTotalPages() > 1}>
              <div class="flex items-center justify-between px-5 py-3 border-t border-slate-100 shrink-0">
                <p class="text-[12px] text-slate-400">{tradesPage()} / {tradesTotalPages()}</p>
                <div class="flex gap-2">
                  <button
                    class="px-3 py-1 text-[12px] border border-slate-200 rounded-lg text-slate-500 hover:bg-slate-50 hover:text-slate-700 disabled:opacity-30 transition-colors font-medium"
                    disabled={tradesPage() <= 1}
                    onClick={() => loadTrades(tradesBotId()!, tradesPage() - 1)}
                  >
                    上一页
                  </button>
                  <button
                    class="px-3 py-1 text-[12px] border border-slate-200 rounded-lg text-slate-500 hover:bg-slate-50 hover:text-slate-700 disabled:opacity-30 transition-colors font-medium"
                    disabled={tradesPage() >= tradesTotalPages()}
                    onClick={() => loadTrades(tradesBotId()!, tradesPage() + 1)}
                  >
                    下一页
                  </button>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </Show>

      {/* ═══ 历史分析记录 ═══ */}
      <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
        <div class="px-5 py-4 border-b border-slate-100">
          <div class="flex items-center gap-3">
            <div class="w-1 h-5 bg-indigo-500 rounded-full" />
            <h3 class="text-[14px] font-semibold text-slate-800">历史分析记录</h3>
          </div>
        </div>
        <div class="text-center py-16">
          <p class="text-slate-400 text-[13px]">暂无历史分析记录</p>
        </div>
      </div>
    </div>
  )
}

export default GridBotPage
