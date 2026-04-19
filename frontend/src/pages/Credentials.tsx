import { type Component, createSignal, Show, For } from 'solid-js'
import { api } from '../lib/api'

// ---- 类型定义 ----

interface Credential {
  id: string
  exchange: string
  label?: string
  created_at: string
  updated_at: string
}

interface SaveCredentialRequest {
  exchange: string
  label?: string
  api_key: string
  api_secret: string
  passphrase?: string
}

interface TestCredentialRequest {
  exchange: string
  api_key: string
  api_secret: string
  passphrase?: string
}

// ---- 交易所配置 ----

const EXCHANGE_CONFIG: Record<string, { name: string; color: string; bg: string; border: string }> = {
  binance: { name: 'Binance', color: 'text-yellow-500', bg: 'bg-yellow-50', border: 'border-yellow-200' },
  okx: { name: 'OKX', color: 'text-gray-900', bg: 'bg-gray-50', border: 'border-gray-300' },
  bybit: { name: 'Bybit', color: 'text-orange-500', bg: 'bg-orange-50', border: 'border-orange-200' },
}

function getExchangeConfig(exchange: string) {
  return EXCHANGE_CONFIG[exchange] || { name: exchange, color: 'text-gray-600', bg: 'bg-gray-50', border: 'border-gray-200' }
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
      const res = await api.get<Credential[]>('/credentials/list')
      if (res.success && res.data) {
        setCredentials(res.data)
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
          <h2 class="text-lg font-semibold text-gray-800">凭证管理</h2>
          <p class="text-sm text-gray-500 mt-1">管理交易所 API 凭证</p>
        </div>
        <button
          onClick={openModal}
          class="px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
        >
          添加凭证
        </button>
      </div>

      {/* 加载中 */}
      <Show when={loading()}>
        <div class="flex items-center justify-center py-16">
          <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
          <span class="ml-3 text-gray-500">加载中...</span>
        </div>
      </Show>

      {/* 错误 */}
      <Show when={!loading() && error()}>
        <div class="bg-red-50 border border-red-200 rounded-xl p-6 text-center">
          <svg class="w-12 h-12 mx-auto text-red-400 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
          </svg>
          <p class="text-red-600">{error()}</p>
          <button
            onClick={fetchCredentials}
            class="mt-3 px-4 py-1.5 text-sm bg-red-100 text-red-700 rounded-lg hover:bg-red-200 transition-colors"
          >
            重试
          </button>
        </div>
      </Show>

      {/* 空状态 */}
      <Show when={!loading() && !error() && credentials().length === 0}>
        <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-12 text-center">
          <svg class="w-16 h-16 mx-auto text-gray-300 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
            <path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
          </svg>
          <p class="text-gray-500">暂无凭证，请添加交易所凭证</p>
          <p class="text-sm text-gray-400 mt-1">添加交易所 API Key 以连接您的交易账户</p>
        </div>
      </Show>

      {/* 凭证卡片网格 */}
      <Show when={!loading() && !error() && credentials().length > 0}>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <For each={credentials()}>
            {(cred) => {
              const config = getExchangeConfig(cred.exchange)
              return (
                <div class={`bg-white rounded-xl shadow-sm border ${config.border} p-5 space-y-4`}>
                  {/* 交易所名称 */}
                  <div class="flex items-center justify-between">
                    <h3 class={`text-xl font-bold ${config.color}`}>{config.name}</h3>
                    <span class="inline-flex items-center gap-1.5 text-sm text-green-600">
                      <span class="w-2 h-2 rounded-full bg-green-500"></span>
                      已配置
                    </span>
                  </div>

                  {/* 标签 */}
                  <Show when={cred.label}>
                    <p class="text-sm text-gray-600 bg-gray-50 rounded-md px-3 py-1.5 inline-block">
                      {cred.label}
                    </p>
                  </Show>

                  {/* 创建时间 */}
                  <p class="text-xs text-gray-400">
                    创建于 {formatDate(cred.created_at)}
                  </p>

                  {/* 卡片测试结果提示 */}
                  <Show when={cardTestResult() && cardTestResult()!.id === cred.id}>
                    <div
                      class={`text-sm rounded-lg px-3 py-2 ${
                        cardTestResult()!.success
                          ? 'bg-green-50 text-green-700 border border-green-200'
                          : 'bg-red-50 text-red-700 border border-red-200'
                      }`}
                    >
                      {cardTestResult()!.message}
                    </div>
                  </Show>

                  {/* 操作按钮 */}
                  <div class="flex gap-2 pt-2">
                    <button
                      onClick={() => handleCardTest(cred)}
                      disabled={cardTestingId() === cred.id}
                      class="flex-1 px-3 py-2 text-sm font-medium text-blue-600 bg-blue-50 rounded-lg hover:bg-blue-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {cardTestingId() === cred.id ? '测试中...' : '测试连接'}
                    </button>
                    <button
                      onClick={() => handleDelete(cred.id)}
                      class="flex-1 px-3 py-2 text-sm font-medium text-red-600 bg-red-50 rounded-lg hover:bg-red-100 transition-colors"
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
          <div class="absolute inset-0 bg-black/50" onClick={closeModal}></div>

          {/* 模态框内容 */}
          <div class="relative bg-white rounded-2xl shadow-xl w-full max-w-md mx-4 p-6 space-y-5">
            {/* 标题 */}
            <div class="flex items-center justify-between">
              <h3 class="text-lg font-semibold text-gray-800">添加凭证</h3>
              <button
                onClick={closeModal}
                class="text-gray-400 hover:text-gray-600 transition-colors"
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
                <label class="block text-sm font-medium text-gray-700 mb-1">交易所</label>
                <select
                  value={formExchange()}
                  onChange={(e) => setFormExchange(e.currentTarget.value)}
                  class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white"
                >
                  <option value="binance">Binance</option>
                  <option value="okx">OKX</option>
                  <option value="bybit">Bybit</option>
                </select>
              </div>

              {/* 标签 */}
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">
                  标签 <span class="text-gray-400 font-normal">(可选)</span>
                </label>
                <input
                  type="text"
                  value={formLabel()}
                  onInput={(e) => setFormLabel(e.currentTarget.value)}
                  placeholder="如: 主账户"
                  class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                />
              </div>

              {/* API Key */}
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">API Key</label>
                <input
                  type="password"
                  value={formApiKey()}
                  onInput={(e) => setFormApiKey(e.currentTarget.value)}
                  placeholder="输入 API Key"
                  class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                />
              </div>

              {/* API Secret */}
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">API Secret</label>
                <input
                  type="password"
                  value={formApiSecret()}
                  onInput={(e) => setFormApiSecret(e.currentTarget.value)}
                  placeholder="输入 API Secret"
                  class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                />
              </div>

              {/* Passphrase (仅 OKX) */}
              <Show when={formExchange() === 'okx'}>
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">
                    Passphrase <span class="text-gray-400 font-normal">(OKX 必填)</span>
                  </label>
                  <input
                    type="password"
                    value={formPassphrase()}
                    onInput={(e) => setFormPassphrase(e.currentTarget.value)}
                    placeholder="输入 Passphrase"
                    class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                  />
                </div>
              </Show>
            </div>

            {/* 测试结果 */}
            <Show when={testResult()}>
              <div
                class={`text-sm rounded-lg px-3 py-2 ${
                  testResult()!.success
                    ? 'bg-green-50 text-green-700 border border-green-200'
                    : 'bg-red-50 text-red-700 border border-red-200'
                }`}
              >
                {testResult()!.message}
              </div>
            </Show>

            {/* 错误 */}
            <Show when={formError()}>
              <div class="text-sm rounded-lg px-3 py-2 bg-red-50 text-red-700 border border-red-200">
                {formError()}
              </div>
            </Show>

            {/* 按钮 */}
            <div class="flex gap-3 pt-2">
              <button
                onClick={handleTest}
                disabled={testing()}
                class="flex-1 px-4 py-2 text-sm font-medium text-blue-600 bg-blue-50 rounded-lg hover:bg-blue-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {testing() ? '测试中...' : '测试连接'}
              </button>
              <button
                onClick={handleSave}
                disabled={saving()}
                class="flex-1 px-4 py-2 text-sm font-medium text-white bg-green-600 rounded-lg hover:bg-green-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
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
