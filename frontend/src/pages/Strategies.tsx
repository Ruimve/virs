import { type Component, createSignal, createEffect, Show, For } from 'solid-js'
import { api } from '../lib/api'
import type { PaginatedResponse } from '../lib/api'

// ── 类型定义 ──────────────────────────────────────────────
interface Strategy {
  id: string
  user_id: string
  name: string
  description?: string
  strategy_type: string
  market_type: 'spot' | 'futures' | 'perpetual'
  symbol: string
  exchange: string
  timeframe: string
  strategy_mode: 'signal' | 'script'
  execution_mode: 'signal_only' | 'live'
  indicator_config: Record<string, unknown>
  trading_config: Record<string, unknown>
  status: 'draft' | 'running' | 'paused' | 'stopped' | 'error'
  decide_interval_secs: number
  created_at: string
  updated_at: string
}

type StatusFilter = 'all' | 'running' | 'stopped' | 'draft'

interface StrategyFormData {
  name: string
  symbol: string
  exchange: string
  timeframe: string
  strategy_type: string
  market_type: string
  execution_mode: string
  indicator_config: string
  trading_config: string
}

const DEFAULT_TRADING_CONFIG = JSON.stringify(
  { fixed_amount: 100, max_position_size: 1000, balance_pct: 0.1, allow_short: false },
  null,
  2
)

const EMPTY_FORM: StrategyFormData = {
  name: '',
  symbol: '',
  exchange: 'binance',
  timeframe: '1h',
  strategy_type: 'sma_crossover',
  market_type: 'spot',
  execution_mode: 'signal_only',
  indicator_config: '{}',
  trading_config: DEFAULT_TRADING_CONFIG,
}

// ── 状态 badge 样式 ──────────────────────────────────────
function statusBadgeClass(status: string): string {
  switch (status) {
    case 'running':
      return 'bg-green-100 text-green-700'
    case 'stopped':
      return 'bg-gray-100 text-gray-600'
    case 'draft':
      return 'bg-yellow-100 text-yellow-700'
    case 'error':
      return 'bg-red-100 text-red-700'
    case 'paused':
      return 'bg-blue-100 text-blue-700'
    default:
      return 'bg-gray-100 text-gray-600'
  }
}

function statusLabel(status: string): string {
  const map: Record<string, string> = {
    running: '运行中',
    stopped: '已停止',
    draft: '草稿',
    error: '错误',
    paused: '已暂停',
  }
  return map[status] || status
}

// ── 组件 ──────────────────────────────────────────────────
const Strategies: Component = () => {
  // 列表状态
  const [strategies, setStrategies] = createSignal<Strategy[]>([])
  const [loading, setLoading] = createSignal(true)
  const [error, setError] = createSignal('')
  const [page, setPage] = createSignal(1)
  const [totalPages, setTotalPages] = createSignal(1)
  const [statusFilter, setStatusFilter] = createSignal<StatusFilter>('all')

  // 模态框状态
  const [showModal, setShowModal] = createSignal(false)
  const [editingId, setEditingId] = createSignal<string | null>(null)
  const [form, setForm] = createSignal<StrategyFormData>({ ...EMPTY_FORM })
  const [formError, setFormError] = createSignal('')
  const [submitting, setSubmitting] = createSignal(false)

  // 操作状态
  const [actionLoading, setActionLoading] = createSignal<string | null>(null)

  // ── 加载策略列表 ──
  async function fetchStrategies() {
    setLoading(true)
    setError('')
    try {
      const params = new URLSearchParams({ page: String(page()), page_size: '20' })
      const res = await api.get<PaginatedResponse<Strategy>>(`/strategies?${params}`)
      if (res.success && res.data) {
        let items = res.data.items
        // 前端过滤状态
        const filter = statusFilter()
        if (filter !== 'all') {
          items = items.filter((s) => s.status === filter)
        }
        setStrategies(items)
        setTotalPages(res.data.total_pages)
      } else {
        setError(res.error || '加载策略列表失败')
        setStrategies([])
      }
    } catch (e: any) {
      setError(e.message || '网络错误')
      setStrategies([])
    } finally {
      setLoading(false)
    }
  }

  createEffect(() => {
    fetchStrategies()
  })

  // 状态筛选变化时回到第一页
  createEffect(() => {
    statusFilter()
    setPage(1)
    fetchStrategies()
  })

  // ── 分页 ──
  function goToPage(p: number) {
    if (p < 1 || p > totalPages()) return
    setPage(p)
    fetchStrategies()
  }

  // ── 打开创建模态框 ──
  function openCreateModal() {
    setEditingId(null)
    setForm({ ...EMPTY_FORM })
    setFormError('')
    setShowModal(true)
  }

  // ── 打开编辑模态框 ──
  async function openEditModal(id: string) {
    setFormError('')
    try {
      const res = await api.get<Strategy>(`/strategies/${id}`)
      if (res.success && res.data) {
        const s = res.data
        setForm({
          name: s.name,
          symbol: s.symbol,
          exchange: s.exchange,
          timeframe: s.timeframe,
          strategy_type: s.strategy_type,
          market_type: s.market_type,
          execution_mode: s.execution_mode,
          indicator_config: JSON.stringify(s.indicator_config || {}, null, 2),
          trading_config: JSON.stringify(s.trading_config || {}, null, 2),
        })
        setEditingId(id)
        setShowModal(true)
      } else {
        alert(res.error || '获取策略详情失败')
      }
    } catch (e: any) {
      alert(e.message || '网络错误')
    }
  }

  // ── 关闭模态框 ──
  function closeModal() {
    setShowModal(false)
    setEditingId(null)
    setFormError('')
  }

  // ── 表单字段更新 ──
  function updateForm(field: keyof StrategyFormData, value: string) {
    setForm((prev) => ({ ...prev, [field]: value }))
  }

  // ── 提交表单 (创建 / 编辑) ──
  async function handleSubmit(e: Event) {
    e.preventDefault()
    setFormError('')

    const f = form()

    // 验证必填
    if (!f.name.trim()) {
      setFormError('策略名称不能为空')
      return
    }
    if (!f.symbol.trim()) {
      setFormError('交易对不能为空')
      return
    }

    // 验证 JSON
    let indicatorConfig: Record<string, unknown>
    let tradingConfig: Record<string, unknown>
    try {
      indicatorConfig = JSON.parse(f.indicator_config)
    } catch {
      setFormError('指标配置 JSON 格式不正确')
      return
    }
    try {
      tradingConfig = JSON.parse(f.trading_config)
    } catch {
      setFormError('交易配置 JSON 格式不正确')
      return
    }

    setSubmitting(true)
    try {
      const payload = {
        name: f.name.trim(),
        symbol: f.symbol.trim(),
        exchange: f.exchange,
        timeframe: f.timeframe,
        strategy_type: f.strategy_type,
        market_type: f.market_type,
        execution_mode: f.execution_mode,
        indicator_config: indicatorConfig,
        trading_config: tradingConfig,
      }

      const eid = editingId()
      let res
      if (eid) {
        res = await api.put<Strategy>(`/strategies/${eid}/update`, payload)
      } else {
        res = await api.post<Strategy>('/strategies/create', payload)
      }

      if (res.success) {
        closeModal()
        await fetchStrategies()
      } else {
        setFormError(res.error || '保存失败')
      }
    } catch (e: any) {
      setFormError(e.message || '网络错误')
    } finally {
      setSubmitting(false)
    }
  }

  // ── 删除策略 ──
  async function handleDelete(id: string) {
    if (!window.confirm('确定要删除该策略吗？此操作不可撤销。')) return
    setActionLoading(id)
    try {
      const res = await api.del(`/strategies/${id}/delete`)
      if (res.success) {
        await fetchStrategies()
      } else {
        alert(res.error || '删除失败')
      }
    } catch (e: any) {
      alert(e.message || '网络错误')
    } finally {
      setActionLoading(null)
    }
  }

  // ── 启动策略 ──
  async function handleStart(id: string) {
    setActionLoading(id)
    try {
      const res = await api.post(`/strategies/${id}/start`)
      if (res.success) {
        await fetchStrategies()
      } else {
        alert(res.error || '启动失败')
      }
    } catch (e: any) {
      alert(e.message || '网络错误')
    } finally {
      setActionLoading(null)
    }
  }

  // ── 停止策略 ──
  async function handleStop(id: string) {
    setActionLoading(id)
    try {
      const res = await api.post(`/strategies/${id}/stop`)
      if (res.success) {
        await fetchStrategies()
      } else {
        alert(res.error || '停止失败')
      }
    } catch (e: any) {
      alert(e.message || '网络错误')
    } finally {
      setActionLoading(null)
    }
  }

  // ── 渲染 ──
  return (
    <div class="space-y-6">
      {/* 标题栏 */}
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-800">策略列表</h2>
          <p class="text-sm text-gray-500 mt-1">管理和监控您的量化交易策略</p>
        </div>
        <button
          class="px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
          onClick={openCreateModal}
        >
          新建策略
        </button>
      </div>

      {/* 状态筛选 Tabs */}
      <div class="flex gap-1 bg-gray-100 rounded-lg p-1 w-fit">
        {([
          ['all', '全部'],
          ['running', '运行中'],
          ['stopped', '已停止'],
          ['draft', '草稿'],
        ] as [StatusFilter, string][]).map(([key, label]) => (
          <button
            class={`px-4 py-1.5 text-sm rounded-md transition-colors ${
              statusFilter() === key
                ? 'bg-white text-gray-800 shadow-sm font-medium'
                : 'text-gray-500 hover:text-gray-700'
            }`}
            onClick={() => setStatusFilter(key)}
          >
            {label}
          </button>
        ))}
      </div>

      {/* 内容区 */}
      <Show
        when={!loading()}
        fallback={
          <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-12 text-center">
            <p class="text-gray-500">加载中...</p>
          </div>
        }
      >
        <Show
          when={error() === ''}
          fallback={
            <div class="bg-red-50 border border-red-200 rounded-xl p-6 text-center">
              <p class="text-red-600">{error()}</p>
              <button
                class="mt-3 px-4 py-1.5 text-sm bg-red-100 text-red-700 rounded-lg hover:bg-red-200 transition-colors"
                onClick={fetchStrategies}
              >
                重试
              </button>
            </div>
          }
        >
          <Show
            when={strategies().length > 0}
            fallback={
              <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-12 text-center">
                <svg
                  class="w-16 h-16 mx-auto text-gray-300 mb-4"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  stroke-width="1"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
                  />
                </svg>
                <p class="text-gray-500">暂无策略</p>
                <p class="text-sm text-gray-400 mt-1">点击"新建策略"开始创建您的第一个交易策略</p>
              </div>
            }
          >
            {/* 策略表格 */}
            <div class="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div class="overflow-x-auto">
                <table class="w-full text-sm">
                  <thead>
                    <tr class="border-b border-gray-200 bg-gray-50">
                      <th class="text-left px-4 py-3 font-medium text-gray-600">名称</th>
                      <th class="text-left px-4 py-3 font-medium text-gray-600">交易对</th>
                      <th class="text-left px-4 py-3 font-medium text-gray-600">交易所</th>
                      <th class="text-left px-4 py-3 font-medium text-gray-600">时间框架</th>
                      <th class="text-left px-4 py-3 font-medium text-gray-600">状态</th>
                      <th class="text-left px-4 py-3 font-medium text-gray-600">执行模式</th>
                      <th class="text-right px-4 py-3 font-medium text-gray-600">操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={strategies()}>
                      {(s) => (
                        <tr class="border-b border-gray-100 hover:bg-gray-50 transition-colors">
                          <td class="px-4 py-3 font-medium text-gray-800">{s.name}</td>
                          <td class="px-4 py-3 text-gray-600">{s.symbol}</td>
                          <td class="px-4 py-3 text-gray-600">{s.exchange}</td>
                          <td class="px-4 py-3 text-gray-600">{s.timeframe}</td>
                          <td class="px-4 py-3">
                            <span
                              class={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${statusBadgeClass(s.status)}`}
                            >
                              {statusLabel(s.status)}
                            </span>
                          </td>
                          <td class="px-4 py-3 text-gray-600">
                            {s.execution_mode === 'live' ? '实盘' : '仅信号'}
                          </td>
                          <td class="px-4 py-3 text-right">
                            <div class="flex items-center justify-end gap-2">
                              {/* 启动 / 停止 */}
                              <Show
                                when={s.status === 'running'}
                                fallback={
                                  <button
                                    class="px-3 py-1 text-xs font-medium text-green-700 bg-green-50 border border-green-200 rounded-md hover:bg-green-100 transition-colors disabled:opacity-50"
                                    disabled={actionLoading() === s.id}
                                    onClick={() => handleStart(s.id)}
                                  >
                                    {actionLoading() === s.id ? '...' : '启动'}
                                  </button>
                                }
                              >
                                <button
                                  class="px-3 py-1 text-xs font-medium text-red-700 bg-red-50 border border-red-200 rounded-md hover:bg-red-100 transition-colors disabled:opacity-50"
                                  disabled={actionLoading() === s.id}
                                  onClick={() => handleStop(s.id)}
                                >
                                  {actionLoading() === s.id ? '...' : '停止'}
                                </button>
                              </Show>
                              {/* 编辑 */}
                              <button
                                class="px-3 py-1 text-xs font-medium text-blue-700 bg-blue-50 border border-blue-200 rounded-md hover:bg-blue-100 transition-colors"
                                onClick={() => openEditModal(s.id)}
                              >
                                编辑
                              </button>
                              {/* 删除 */}
                              <button
                                class="px-3 py-1 text-xs font-medium text-gray-600 bg-gray-50 border border-gray-200 rounded-md hover:bg-gray-100 transition-colors disabled:opacity-50"
                                disabled={actionLoading() === s.id}
                                onClick={() => handleDelete(s.id)}
                              >
                                删除
                              </button>
                            </div>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </div>

              {/* 分页 */}
              <Show when={totalPages() > 1}>
                <div class="flex items-center justify-between px-4 py-3 border-t border-gray-200 bg-gray-50">
                  <button
                    class="px-3 py-1.5 text-sm text-gray-600 bg-white border border-gray-200 rounded-md hover:bg-gray-50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    disabled={page() <= 1}
                    onClick={() => goToPage(page() - 1)}
                  >
                    上一页
                  </button>
                  <span class="text-sm text-gray-600">
                    第 {page()} / {totalPages()} 页
                  </span>
                  <button
                    class="px-3 py-1.5 text-sm text-gray-600 bg-white border border-gray-200 rounded-md hover:bg-gray-50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    disabled={page() >= totalPages()}
                    onClick={() => goToPage(page() + 1)}
                  >
                    下一页
                  </button>
                </div>
              </Show>
            </div>
          </Show>
        </Show>
      </Show>

      {/* 创建 / 编辑 模态框 */}
      <Show when={showModal()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center">
          {/* 遮罩 */}
          <div class="absolute inset-0 bg-black/40" onClick={closeModal} />
          {/* 模态框 */}
          <div class="relative bg-white rounded-xl shadow-xl w-full max-w-2xl max-h-[90vh] overflow-y-auto mx-4">
            <div class="px-6 py-4 border-b border-gray-200">
              <h3 class="text-lg font-semibold text-gray-800">
                {editingId() ? '编辑策略' : '新建策略'}
              </h3>
            </div>
            <form onSubmit={handleSubmit}>
              <div class="px-6 py-4 space-y-4">
                {/* 错误提示 */}
                <Show when={formError()}>
                  <div class="p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-600">
                    {formError()}
                  </div>
                </Show>

                {/* 名称 */}
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">
                    策略名称 <span class="text-red-500">*</span>
                  </label>
                  <input
                    type="text"
                    class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    value={form().name}
                    onInput={(e) => updateForm('name', (e.target as HTMLInputElement).value)}
                    placeholder="输入策略名称"
                  />
                </div>

                {/* 交易对 */}
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">
                    交易对 <span class="text-red-500">*</span>
                  </label>
                  <input
                    type="text"
                    class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    value={form().symbol}
                    onInput={(e) => updateForm('symbol', (e.target as HTMLInputElement).value)}
                    placeholder="例如 BTCUSDT"
                  />
                </div>

                {/* 交易所 + 时间框架 */}
                <div class="grid grid-cols-2 gap-4">
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-1">交易所</label>
                    <select
                      class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      value={form().exchange}
                      onChange={(e) => updateForm('exchange', (e.target as HTMLSelectElement).value)}
                    >
                      <option value="binance">Binance</option>
                      <option value="okx">OKX</option>
                      <option value="bybit">Bybit</option>
                    </select>
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-1">时间框架</label>
                    <select
                      class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      value={form().timeframe}
                      onChange={(e) => updateForm('timeframe', (e.target as HTMLSelectElement).value)}
                    >
                      <option value="1m">1m</option>
                      <option value="5m">5m</option>
                      <option value="15m">15m</option>
                      <option value="1h">1h</option>
                      <option value="4h">4h</option>
                      <option value="1d">1d</option>
                    </select>
                  </div>
                </div>

                {/* 策略类型 + 市场类型 */}
                <div class="grid grid-cols-2 gap-4">
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-1">策略类型</label>
                    <select
                      class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      value={form().strategy_type}
                      onChange={(e) => updateForm('strategy_type', (e.target as HTMLSelectElement).value)}
                    >
                      <option value="sma_crossover">SMA 交叉</option>
                      <option value="rsi">RSI</option>
                      <option value="macd">MACD</option>
                      <option value="bollinger_bands">布林带</option>
                    </select>
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-1">市场类型</label>
                    <select
                      class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      value={form().market_type}
                      onChange={(e) => updateForm('market_type', (e.target as HTMLSelectElement).value)}
                    >
                      <option value="spot">现货</option>
                      <option value="futures">合约</option>
                      <option value="perpetual">永续</option>
                    </select>
                  </div>
                </div>

                {/* 执行模式 */}
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">执行模式</label>
                  <select
                    class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    value={form().execution_mode}
                    onChange={(e) => updateForm('execution_mode', (e.target as HTMLSelectElement).value)}
                  >
                    <option value="signal_only">仅信号</option>
                    <option value="live">实盘</option>
                  </select>
                </div>

                {/* 指标配置 */}
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">指标配置 (JSON)</label>
                  <textarea
                    class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm font-mono focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    rows="4"
                    value={form().indicator_config}
                    onInput={(e) => updateForm('indicator_config', (e.target as HTMLTextAreaElement).value)}
                    placeholder='{"period": 14}'
                  />
                </div>

                {/* 交易配置 */}
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">交易配置 (JSON)</label>
                  <textarea
                    class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm font-mono focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    rows="4"
                    value={form().trading_config}
                    onInput={(e) => updateForm('trading_config', (e.target as HTMLTextAreaElement).value)}
                  />
                </div>
              </div>

              {/* 底部按钮 */}
              <div class="px-6 py-4 border-t border-gray-200 flex justify-end gap-3">
                <button
                  type="button"
                  class="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors"
                  onClick={closeModal}
                >
                  取消
                </button>
                <button
                  type="submit"
                  class="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
                  disabled={submitting()}
                >
                  {submitting() ? '保存中...' : '保存'}
                </button>
              </div>
            </form>
          </div>
        </div>
      </Show>
    </div>
  )
}

export default Strategies
