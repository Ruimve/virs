import { type Component, createSignal, Show, For, onMount } from 'solid-js'
import { api } from '../lib/api'

// ---- 类型定义 ----

interface AiCredential {
  id: string
  provider: string
  label?: string
  is_default: boolean
  created_at: string
  updated_at: string
}

// ---- Provider 配置 ----

const PROVIDER_CONFIG: Record<string, { name: string; color: string; icon: string; desc: string }> = {
  openrouter: { name: 'OpenRouter', color: 'bg-purple-100 text-purple-700', icon: 'OR', desc: '支持多种模型（Gemini、GPT、Claude 等）' },
  openai: { name: 'OpenAI', color: 'bg-green-100 text-green-700', icon: 'OA', desc: 'GPT-4o、GPT-4o-mini 等' },
  deepseek: { name: 'DeepSeek', color: 'bg-blue-100 text-blue-700', icon: 'DS', desc: 'DeepSeek-Chat、DeepSeek-Coder' },
}

function getProviderConfig(provider: string) {
  return PROVIDER_CONFIG[provider] || { name: provider, color: 'bg-gray-100 text-gray-600', icon: provider[0]?.toUpperCase() || '?', desc: '' }
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

const AiCredentials: Component = () => {
  const [credentials, setCredentials] = createSignal<AiCredential[]>([])
  const [loading, setLoading] = createSignal(true)
  const [error, setError] = createSignal<string | null>(null)

  // 模态框
  const [showModal, setShowModal] = createSignal(false)
  const [saving, setSaving] = createSignal(false)
  const [testing, setTesting] = createSignal(false)
  const [testResult, setTestResult] = createSignal<{ success: boolean; message: string } | null>(null)

  // 表单
  const [formProvider, setFormProvider] = createSignal('openrouter')
  const [formLabel, setFormLabel] = createSignal('')
  const [formApiKey, setFormApiKey] = createSignal('')
  const [formError, setFormError] = createSignal<string | null>(null)

  // 删除确认
  const [deletingId, setDeletingId] = createSignal<string | null>(null)

  // 获取凭证列表
  async function fetchCredentials() {
    setLoading(true)
    setError(null)
    try {
      const res = await api.get<{ items: AiCredential[] }>('/ai-credentials/list')
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

  onMount(fetchCredentials)

  // 打开模态框
  function openModal() {
    setFormProvider('openrouter')
    setFormLabel('')
    setFormApiKey('')
    setFormError(null)
    setTestResult(null)
    setShowModal(true)
  }

  function closeModal() {
    setShowModal(false)
    setFormError(null)
    setTestResult(null)
  }

  // 测试连接
  async function handleTest() {
    const apiKey = formApiKey().trim()
    if (!apiKey) {
      setFormError('请输入 API Key')
      return
    }
    setTesting(true)
    setTestResult(null)
    setFormError(null)
    try {
      const res = await api.post('/ai-credentials/test', {
        provider: formProvider(),
        api_key: apiKey,
      })
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
    if (!apiKey) {
      setFormError('请输入 API Key')
      return
    }
    setSaving(true)
    setFormError(null)
    try {
      const res = await api.post('/ai-credentials/save', {
        provider: formProvider(),
        api_key: apiKey,
        label: formLabel().trim() || undefined,
      })
      if (res.success) {
        closeModal()
        fetchCredentials()
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
    setDeletingId(id)
    try {
      const res = await api.post(`/ai-credentials/delete/${id}`)
      if (res.success) {
        setCredentials(prev => prev.filter(c => c.id !== id))
      }
    } catch {
      // ignore
    } finally {
      setDeletingId(null)
    }
  }

  return (
    <div class="p-6 space-y-6">
      {/* 页面标题 */}
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-xl font-semibold text-gray-900">AI 模型凭证</h1>
          <p class="text-sm text-gray-500 mt-1">配置 AI 策略生成服务所需的 API Key，凭证加密存储</p>
        </div>
        <button
          class="px-4 py-2 bg-blue-500 text-white text-sm font-medium rounded-lg hover:bg-blue-600 transition-colors"
          onClick={openModal}
        >
          + 添加凭证
        </button>
      </div>

      {/* 提示信息 */}
      <div class="bg-blue-50 border border-blue-200/60 rounded-lg p-4">
        <div class="flex">
          <div class="text-blue-500 mt-0.5">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
              <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
          <div class="ml-3">
            <p class="text-sm text-blue-700">
              每个用户可独立配置 AI 凭证。未配置时将使用系统默认凭证。支持 OpenRouter、OpenAI、DeepSeek 三个服务。
            </p>
          </div>
        </div>
      </div>

      {/* 加载状态 */}
      <Show when={loading()}>
        <div class="text-center py-12 text-gray-400">加载中...</div>
      </Show>

      {/* 错误提示 */}
      <Show when={!loading() && error()}>
        <div class="bg-red-50 border border-red-200 rounded-lg p-4 text-red-600 text-sm">{error()}</div>
      </Show>

      {/* 凭证列表 */}
      <Show when={!loading() && !error()}>
        <div class="grid gap-4">
          <For each={credentials()}>
            {(cred) => {
              const config = getProviderConfig(cred.provider)
              return (
                <div class="bg-white rounded-xl border border-gray-200/60 p-5 hover:shadow-sm transition-shadow">
                  <div class="flex items-start justify-between">
                    <div class="flex items-start gap-4">
                      <div class={`w-10 h-10 rounded-lg flex items-center justify-center text-sm font-bold ${config.color}`}>
                        {config.icon}
                      </div>
                      <div>
                        <div class="flex items-center gap-2">
                          <h3 class="text-[15px] font-semibold text-gray-800">{config.name}</h3>
                          {cred.is_default && (
                            <span class="px-2 py-0.5 rounded-md text-[11px] font-medium bg-blue-50 text-blue-600">默认</span>
                          )}
                        </div>
                        <p class="text-[13px] text-gray-500 mt-0.5">{config.desc}</p>
                        {cred.label && (
                          <p class="text-[13px] text-gray-400 mt-1">备注: {cred.label}</p>
                        )}
                        <p class="text-[12px] text-gray-400 mt-1">
                          更新于 {formatDate(cred.updated_at)}
                        </p>
                      </div>
                    </div>
                    <button
                      class="text-gray-400 hover:text-red-500 transition-colors p-1"
                      onClick={() => handleDelete(cred.id)}
                      disabled={deletingId() === cred.id}
                    >
                      <Show when={deletingId() !== cred.id} fallback={
                        <div class="w-4 h-4 border-2 border-red-300 border-t-transparent rounded-full animate-spin" />
                      }>
                        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                          <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </Show>
                    </button>
                  </div>
                </div>
              )
            }}
          </For>

          {/* 空状态 */}
          <Show when={credentials().length === 0}>
            <div class="text-center py-16">
              <div class="text-4xl mb-3">🔑</div>
              <p class="text-gray-500 text-sm">暂无 AI 凭证配置</p>
              <p class="text-gray-400 text-[13px] mt-1">点击上方按钮添加凭证以使用 AI 策略生成功能</p>
            </div>
          </Show>
        </div>
      </Show>

      {/* 添加/编辑模态框 */}
      <Show when={showModal()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center">
          <div class="fixed inset-0 bg-black/40" onClick={closeModal} />
          <div class="relative bg-white rounded-2xl shadow-xl w-full max-w-md mx-4 p-6">
            <h2 class="text-lg font-semibold text-gray-900 mb-4">添加 AI 凭证</h2>

            {/* Provider 选择 */}
            <div class="mb-4">
              <label class="block text-[13px] font-medium text-gray-600 mb-1.5">服务商</label>
              <div class="grid grid-cols-3 gap-2">
                <For each={Object.entries(PROVIDER_CONFIG)}>
                  {([key, cfg]) => (
                    <button
                      class={`p-3 rounded-lg border-2 text-center transition-all ${
                        formProvider() === key
                          ? 'border-blue-500 bg-blue-50'
                          : 'border-gray-200 hover:border-gray-300'
                      }`}
                      onClick={() => setFormProvider(key)}
                    >
                      <div class={`text-sm font-bold ${formProvider() === key ? 'text-blue-600' : 'text-gray-600'}`}>
                        {cfg.icon}
                      </div>
                      <div class="text-[12px] mt-1 text-gray-500">{cfg.name}</div>
                    </button>
                  )}
                </For>
              </div>
            </div>

            {/* API Key */}
            <div class="mb-4">
              <label class="block text-[13px] font-medium text-gray-600 mb-1.5">API Key</label>
              <input
                type="password"
                class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                placeholder="sk-..."
                value={formApiKey()}
                onInput={(e) => setFormApiKey(e.currentTarget.value)}
              />
            </div>

            {/* Label */}
            <div class="mb-4">
              <label class="block text-[13px] font-medium text-gray-600 mb-1.5">备注（可选）</label>
              <input
                type="text"
                class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                placeholder="例如：个人账号"
                value={formLabel()}
                onInput={(e) => setFormLabel(e.currentTarget.value)}
              />
            </div>

            {/* 错误提示 */}
            <Show when={formError()}>
              <div class="mb-4 p-3 bg-red-50 border border-red-200 rounded-lg text-red-600 text-sm">{formError()}</div>
            </Show>

            {/* 测试结果 */}
            <Show when={testResult()}>
              <div class={`mb-4 p-3 rounded-lg text-sm ${
                testResult()!.success
                  ? 'bg-emerald-50 border border-emerald-200 text-emerald-600'
                  : 'bg-red-50 border border-red-200 text-red-600'
              }`}>
                {testResult()!.success ? '✓ ' : '✗ '}{testResult()!.message}
              </div>
            </Show>

            {/* 按钮 */}
            <div class="flex items-center justify-end gap-3 mt-6">
              <button
                class="px-4 py-2 text-sm text-gray-600 hover:text-gray-800 transition-colors"
                onClick={closeModal}
              >
                取消
              </button>
              <button
                class="px-4 py-2 text-sm border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors"
                onClick={handleTest}
                disabled={testing()}
              >
                {testing() ? '测试中...' : '测试连接'}
              </button>
              <button
                class="px-4 py-2 text-sm bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors disabled:opacity-50"
                onClick={handleSave}
                disabled={saving()}
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

export default AiCredentials
