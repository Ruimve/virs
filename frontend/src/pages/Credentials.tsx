import { type Component, createSignal, Show, For } from 'solid-js'
import { api } from '../lib/api'
import { useMarket } from '../lib/market-context'

// ---- 类型定义 ----

interface Credential {
  id: string
  exchange: string
  market_type: string
  label?: string
  created_at: string
}

interface SaveCredentialRequest {
  exchange: string
  market_type: string
  label?: string
  api_key: string
  api_secret: string
  passphrase?: string
}

interface TestCredentialRequest {
  exchange: string
  market_type: string
  api_key: string
  api_secret: string
  passphrase?: string
}

// ---- 交易所配置 ----

const EXCHANGE_CONFIG: Record<string, { name: string; color: string; icon: string }> = {
  binance: { name: 'Binance', color: 'text-yellow-500', icon: 'B' },
  okx: { name: 'OKX', color: 'text-gray-900', icon: 'O' },
  bybit: { name: 'Bybit', color: 'text-orange-500', icon: 'By' },
}

function getExchangeConfig(exchange: string) {
  return EXCHANGE_CONFIG[exchange] || { name: exchange, color: 'text-gray-600', icon: exchange[0]?.toUpperCase() || '?' }
}

function formatDate(dateStr: string): string {
  try {
    const d = new Date(dateStr)
    return d.toLocaleDateString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
  } catch {
    return dateStr
  }
}

// ---- 组件 ----

const Credentials: Component = () => {
  const market = useMarket()
  const [credentials, setCredentials] = createSignal<Credential[]>([])
  const [loading, setLoading] = createSignal(true)
  const [error, setError] = createSignal<string | null>(null)

  // 模态框
  const [showModal, setShowModal] = createSignal(false)
  const [saving, setSaving] = createSignal(false)
  const [testing, setTesting] = createSignal(false)
  const [testResult, setTestResult] = createSignal<{ success: boolean; message: string } | null>(null)

  // 表单
  const [formExchange, setFormExchange] = createSignal('binance')
  const [formMarketType, setFormMarketType] = createSignal('perpetual')
  const [formLabel, setFormLabel] = createSignal('')
  const [formApiKey, setFormApiKey] = createSignal('')
  const [formApiSecret, setFormApiSecret] = createSignal('')
  const [formPassphrase, setFormPassphrase] = createSignal('')
  const [formError, setFormError] = createSignal<string | null>(null)

  // 卡片测试
  const [cardTestingId, setCardTestingId] = createSignal<string | null>(null)
  const [cardTestResult, setCardTestResult] = createSignal<{ id: string; success: boolean; message: string } | null>(null)

  // 获取凭证列表
  async function fetchCredentials() {
    setLoading(true)
    setError(null)
    try {
      const res = await api.get<{ items: Credential[] }>('/credentials/list')
      if (res.success && res.data) {
        setCredentials(res.data.items || [])
      } else {
        setError(res.error || '获取凭证列表失败')
      }
    } catch (e: any) {
      setError(e.message || '网络错误')
    } finally {
      setLoading(false)
    }
  }

  // 打开模态框
  function openModal() {
    setFormExchange('binance')
    setFormMarketType(market.marketType)
    setFormLabel('')
    setFormApiKey('')
    setFormApiSecret('')
    setFormPassphrase('')
    setFormError(null)
    setTestResult(null)
    setShowModal(true)
  }

  // 关闭模态框
  function closeModal() {
    setShowModal(false)
    setFormError(null)
    setTestResult(null)
  }

  // 测试连接 (模态框内)
  async function handleTest() {
    const apiKey = formApiKey().trim()
    const apiSecret = formApiSecret().trim()
    if (!apiKey || !apiSecret) {
      setFormError('请输入 API Key 和 API Secret')
      return
    }
    setTesting(true)
    setTestResult(null)
    setFormError(null)
    try {
      const req: TestCredentialRequest = {
        exchange: formExchange(),
        market_type: formMarketType(),
        api_key: apiKey,
        api_secret: apiSecret,
      }
      if (formExchange() === 'okx' && formPassphrase().trim()) {
        req.passphrase = formPassphrase().trim()
      }
      const res = await api.post('/credentials/test', req)
      if (res.success) {
        setTestResult({ success: true, message: '连接成功' })
      } else {
        setTestResult({ success: false, message: res.error || '连接失败' })
      }
    } catch (e: any) {
      setTestResult({ success: false, message: e.message || '网络错误' })
    } finally {
      setTesting(false)
    }
  }

  // 保存凭证
  async function handleSave() {
    const apiKey = formApiKey().trim()
    const apiSecret = formApiSecret().trim()
    if (!apiKey || !apiSecret) {
      setFormError('请输入 API Key 和 API Secret')
      return
    }
    setSaving(true)
    setFormError(null)
    try {
      const req: SaveCredentialRequest = {
        exchange: formExchange(),
        market_type: formMarketType(),
        api_key: apiKey,
        api_secret: apiSecret,
      }
      if (formLabel().trim()) {
        req.label = formLabel().trim()
      }
      if (formExchange() === 'okx' && formPassphrase().trim()) {
        req.passphrase = formPassphrase().trim()
      }
      const res = await api.post('/credentials/save', req)
      if (res.success) {
        closeModal()
        await fetchCredentials()
      } else {
        setFormError(res.error || '保存失败')
      }
    } catch (e: any) {
      setFormError(e.message || '网络错误')
    } finally {
      setSaving(false)
    }
  }

  // 删除凭证
  async function handleDelete(id: string) {
    if (!window.confirm('确定要删除此凭证吗？删除后不可恢复。')) return
    try {
      const res = await api.del(`/credentials/delete/${id}`)
      if (res.success) {
        await fetchCredentials()
      } else {
        alert(res.error || '删除失败')
      }
    } catch (e: any) {
      alert(e.message || '网络错误')
    }
  }

  // 卡片测试连接 (弹出输入框)
  async function handleCardTest(cred: Credential) {
    const apiKey = window.prompt('请输入 API Key:')
    if (!apiKey) return
    const apiSecret = window.prompt('请输入 API Secret:')
    if (!apiSecret) return
    let passphrase: string | undefined
    if (cred.exchange === 'okx') {
      passphrase = window.prompt('请输入 Passphrase (可留空):') || undefined
    }

    setCardTestingId(cred.id)
    setCardTestResult(null)
    try {
      const req: TestCredentialRequest = {
        exchange: cred.exchange,
        market_type: cred.market_type,
        api_key: apiKey,
        api_secret: apiSecret,
      }
      if (passphrase) {
        req.passphrase = passphrase
      }
      const res = await api.post('/credentials/test', req)
      if (res.success) {
        setCardTestResult({ id: cred.id, success: true, message: '连接成功' })
      } else {
        setCardTestResult({ id: cred.id, success: false, message: res.error || '连接失败' })
      }
    } catch (e: any) {
      setCardTestResult({ id: cred.id, success: false, message: e.message || '网络错误' })
    } finally {
      setCardTestingId(null)
      // 5 秒后自动清除提示
      setTimeout(() => {
        setCardTestResult(null)
      }, 5000)
    }
  }

  // 初始化加载
  fetchCredentials()

  return (
    <div class="space-y-6">
      {/* 标题栏 */}
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-900">凭证管理</h2>
          <p class="text-sm text-gray-400 mt-1">管理交易所 API 凭证</p>
        </div>
        <button
          onClick={openModal}
          class="px-4 py-2 bg-indigo-600 text-white text-sm font-medium rounded-lg hover:bg-indigo-700 transition-colors"
        >
          添加凭证
        </button>
      </div>

      {/* 加载中 */}
      <Show when={loading()}>
        <div class="flex items-center justify-center py-16">
          <div class="animate-spin rounded-full h-6 w-6 border-2 border-indigo-500 border-t-transparent"></div>
          <span class="ml-3 text-gray-400 text-sm">加载中...</span>
        </div>
      </Show>

      {/* 错误 */}
      <Show when={!loading() && error()}>
        <div class="bg-white rounded-xl border border-red-200/60 p-8 text-center">
          <svg class="w-10 h-10 mx-auto text-red-300 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
          </svg>
          <p class="text-red-500 text-sm">{error()}</p>
          <button
            onClick={fetchCredentials}
            class="mt-3 px-4 py-1.5 text-sm text-red-500 bg-red-50 rounded-lg hover:bg-red-100 transition-colors"
          >
            重试
          </button>
        </div>
      </Show>

      {/* 空状态 */}
      <Show when={!loading() && !error() && credentials().length === 0}>
        <div class="bg-white rounded-xl border border-gray-200/60 p-16 text-center">
          <svg class="w-12 h-12 mx-auto text-gray-200 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
            <path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
          </svg>
          <p class="text-gray-400 text-sm">暂无凭证，请添加交易所凭证</p>
          <p class="text-[13px] text-gray-300 mt-1">添加交易所 API Key 以连接您的交易账户</p>
        </div>
      </Show>

      {/* 凭证卡片网格 */}
      <Show when={!loading() && !error() && credentials().length > 0}>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          <For each={credentials()}>
            {(cred) => {
              const config = getExchangeConfig(cred.exchange)
              return (
                <div class="bg-white rounded-xl border border-gray-200/60 p-5 space-y-4">
                  {/* 交易所名称 */}
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-3">
                      <div class={`w-9 h-9 rounded-lg bg-gray-50 flex items-center justify-center text-sm font-bold ${config.color}`}>
                        {config.icon}
                      </div>
                      <div>
                        <h3 class={`text-[15px] font-semibold ${config.color}`}>{config.name}</h3>
                        <span class="text-[11px] text-gray-400">{cred.market_type === 'perpetual' ? '永续' : '现货'}</span>
                      </div>
                    </div>
                    <span class="inline-flex items-center gap-1.5 text-xs text-emerald-600">
                      <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
                      已配置
                    </span>
                  </div>

                  {/* 标签 */}
                  <Show when={cred.label}>
                    <p class="text-[13px] text-gray-500 bg-gray-50 rounded-md px-3 py-1.5 inline-block">
                      {cred.label}
                    </p>
                  </Show>

                  {/* 创建时间 */}
                  <p class="text-xs text-gray-300">
                    创建于 {formatDate(cred.created_at)}
                  </p>

                  {/* 卡片测试结果提示 */}
                  <Show when={cardTestResult() && cardTestResult()!.id === cred.id}>
                    <div
                      class={`text-[13px] rounded-lg px-3 py-2 ${
                        cardTestResult()!.success
                          ? 'bg-emerald-50 text-emerald-600 border border-emerald-200/60'
                          : 'bg-red-50 text-red-500 border border-red-200/60'
                      }`}
                    >
                      {cardTestResult()!.message}
                    </div>
                  </Show>

                  {/* 操作按钮 */}
                  <div class="flex gap-2 pt-1">
                    <button
                      onClick={() => handleCardTest(cred)}
                      disabled={cardTestingId() === cred.id}
                      class="flex-1 px-3 py-2 text-[13px] font-medium text-indigo-600 bg-indigo-50 rounded-lg hover:bg-indigo-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {cardTestingId() === cred.id ? '测试中...' : '测试连接'}
                    </button>
                    <button
                      onClick={() => handleDelete(cred.id)}
                      class="flex-1 px-3 py-2 text-[13px] font-medium text-gray-500 bg-gray-50 rounded-lg hover:bg-gray-100 transition-colors"
                    >
                      删除
                    </button>
                  </div>
                </div>
              )
            }}
          </For>
        </div>
      </Show>

      {/* 添加凭证模态框 */}
      <Show when={showModal()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center">
          {/* 遮罩 */}
          <div class="absolute inset-0 bg-black/40 backdrop-blur-sm" onClick={closeModal}></div>

          {/* 模态框内容 */}
          <div class="relative bg-white rounded-2xl border border-gray-200/60 w-full max-w-md mx-4 p-6 space-y-5">
            {/* 标题 */}
            <div class="flex items-center justify-between">
              <h3 class="text-[15px] font-semibold text-gray-900">添加凭证</h3>
              <button
                onClick={closeModal}
                class="text-gray-300 hover:text-gray-500 transition-colors"
              >
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            {/* 表单 */}
            <div class="space-y-4">
              {/* 交易所 */}
              <div>
                <label class="block text-[13px] font-medium text-gray-400 mb-1.5">交易所</label>
                <select
                  value={formExchange()}
                  onChange={(e) => setFormExchange(e.currentTarget.value)}
                  class="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 bg-white"
                >
                  <option value="binance">Binance</option>
                  <option value="okx">OKX</option>
                  <option value="bybit">Bybit</option>
                </select>
              </div>

              {/* 市场类型 */}
              <div>
                <label class="block text-[13px] font-medium text-gray-400 mb-1.5">市场类型</label>
                <select
                  value={formMarketType()}
                  onChange={(e) => setFormMarketType(e.currentTarget.value)}
                  class="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 bg-white"
                >
                  <option value="perpetual">永续合约</option>
                  <option value="spot">现货</option>
                </select>
              </div>

              {/* 标签 */}
              <div>
                <label class="block text-[13px] font-medium text-gray-400 mb-1.5">
                  标签 <span class="text-gray-300 font-normal">(可选)</span>
                </label>
                <input
                  type="text"
                  value={formLabel()}
                  onInput={(e) => setFormLabel(e.currentTarget.value)}
                  placeholder="如: 主账户"
                  class="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 placeholder:text-gray-300"
                />
              </div>

              {/* API Key */}
              <div>
                <label class="block text-[13px] font-medium text-gray-400 mb-1.5">API Key</label>
                <input
                  type="password"
                  value={formApiKey()}
                  onInput={(e) => setFormApiKey(e.currentTarget.value)}
                  placeholder="输入 API Key"
                  class="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 placeholder:text-gray-300"
                />
              </div>

              {/* API Secret */}
              <div>
                <label class="block text-[13px] font-medium text-gray-400 mb-1.5">API Secret</label>
                <input
                  type="password"
                  value={formApiSecret()}
                  onInput={(e) => setFormApiSecret(e.currentTarget.value)}
                  placeholder="输入 API Secret"
                  class="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 placeholder:text-gray-300"
                />
              </div>

              {/* Passphrase (仅 OKX) */}
              <Show when={formExchange() === 'okx'}>
                <div>
                  <label class="block text-[13px] font-medium text-gray-400 mb-1.5">
                    Passphrase <span class="text-gray-300 font-normal">(OKX 必填)</span>
                  </label>
                  <input
                    type="password"
                    value={formPassphrase()}
                    onInput={(e) => setFormPassphrase(e.currentTarget.value)}
                    placeholder="输入 Passphrase"
                    class="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-400 placeholder:text-gray-300"
                  />
                </div>
              </Show>
            </div>

            {/* 测试结果 */}
            <Show when={testResult()}>
              <div
                class={`text-[13px] rounded-lg px-3 py-2 ${
                  testResult()!.success
                    ? 'bg-emerald-50 text-emerald-600 border border-emerald-200/60'
                    : 'bg-red-50 text-red-500 border border-red-200/60'
                }`}
              >
                {testResult()!.message}
              </div>
            </Show>

            {/* 错误 */}
            <Show when={formError()}>
              <div class="text-[13px] rounded-lg px-3 py-2 bg-red-50 text-red-500 border border-red-200/60">
                {formError()}
              </div>
            </Show>

            {/* 按钮 */}
            <div class="flex gap-3 pt-1">
              <button
                onClick={handleTest}
                disabled={testing()}
                class="flex-1 px-4 py-2 text-sm font-medium text-indigo-600 bg-indigo-50 rounded-lg hover:bg-indigo-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {testing() ? '测试中...' : '测试连接'}
              </button>
              <button
                onClick={handleSave}
                disabled={saving()}
                class="flex-1 px-4 py-2 text-sm font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {saving() ? '保存中...' : '保存'}
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  )
}

export default Credentials
