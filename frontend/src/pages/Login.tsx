import { type Component, createSignal, Show } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import { login } from '../lib/api'

const Login: Component = () => {
  const navigate = useNavigate()
  const [username, setUsername] = createSignal('')
  const [password, setPassword] = createSignal('')
  const [error, setError] = createSignal('')
  const [loading, setLoading] = createSignal(false)

  const handleSubmit = async (e: Event) => {
    e.preventDefault()
    setError('')

    if (!username().trim() || !password().trim()) {
      setError('请输入用户名和密码')
      return
    }

    setLoading(true)
    try {
      const result = await login(username().trim(), password())
      if (result.success) {
        navigate('/dashboard', { replace: true })
      } else {
        setError(result.error || '登录失败')
      }
    } catch {
      setError('网络错误，请稍后重试')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div class="min-h-screen bg-gray-100 flex items-center justify-center px-4">
      <div class="w-full max-w-md">
        {/* Logo 区域 */}
        <div class="text-center mb-8">
          <h1 class="text-3xl font-bold text-gray-900 tracking-tight">VIRS</h1>
          <p class="mt-2 text-sm text-gray-500">量化交易管理系统</p>
        </div>

        {/* 登录卡片 */}
        <div class="bg-white rounded-xl shadow-lg p-8">
          <h2 class="text-xl font-semibold text-gray-800 mb-6">登录</h2>

          <form onSubmit={handleSubmit} class="space-y-5">
            {/* 错误提示 */}
            <Show when={error()}>
              {(errorMsg) => (
                <div class="flex items-center gap-2 p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-600">
                  <svg class="w-5 h-5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <span>{errorMsg()}</span>
                </div>
              )}
            </Show>

            {/* 用户名 */}
            <div>
              <label for="username" class="block text-sm font-medium text-gray-700 mb-1.5">
                用户名
              </label>
              <input
                id="username"
                type="text"
                value={username()}
                onInput={(e) => setUsername(e.currentTarget.value)}
                class="w-full px-4 py-2.5 border border-gray-300 rounded-lg text-sm text-gray-900 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-shadow"
                placeholder="请输入用户名"
                autocomplete="username"
                disabled={loading()}
              />
            </div>

            {/* 密码 */}
            <div>
              <label for="password" class="block text-sm font-medium text-gray-700 mb-1.5">
                密码
              </label>
              <input
                id="password"
                type="password"
                value={password()}
                onInput={(e) => setPassword(e.currentTarget.value)}
                class="w-full px-4 py-2.5 border border-gray-300 rounded-lg text-sm text-gray-900 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-shadow"
                placeholder="请输入密码"
                autocomplete="current-password"
                disabled={loading()}
              />
            </div>

            {/* 登录按钮 */}
            <button
              type="submit"
              disabled={loading()}
              class="w-full py-2.5 px-4 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed transition-colors duration-150"
            >
              <Show when={loading()} fallback="登录">
                <span class="flex items-center justify-center gap-2">
                  <svg class="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                  </svg>
                  登录中...
                </span>
              </Show>
            </button>
          </form>
        </div>
      </div>
    </div>
  )
}

export default Login
