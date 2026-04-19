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
    <div class="min-h-screen bg-gradient-to-b from-white to-slate-50 flex items-center justify-center px-4 animate-fade-in">
      <div class="w-full max-w-md animate-fade-in-up">
        {/* Logo 区域 */}
        <div class="text-center mb-10">
          <h1 class="text-4xl font-light text-[var(--color-text-primary)] tracking-[0.2em]">VIRS</h1>
          <p class="mt-3 text-xs text-[var(--color-text-tertiary)] tracking-wide">量化交易管理系统</p>
        </div>

        {/* 登录卡片 */}
        <div class="bg-white border border-[var(--color-border)] rounded-2xl p-8 shadow-[0_1px_3px_rgba(0,0,0,0.04)]">
          <h2 class="text-lg font-semibold text-[var(--color-text-primary)] mb-6">登录</h2>

          <form onSubmit={handleSubmit} class="space-y-5">
            {/* 错误提示 */}
            <Show when={error()}>
              {(errorMsg) => (
                <div class="flex items-center gap-2 p-3 bg-red-50/80 border border-red-100 rounded-xl text-sm text-red-600">
                  <svg class="w-4 h-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <span>{errorMsg()}</span>
                </div>
              )}
            </Show>

            {/* 用户名 */}
            <div>
              <label for="username" class="block text-[13px] font-medium text-[var(--color-text-secondary)] mb-1.5">
                用户名
              </label>
              <input
                id="username"
                type="text"
                value={username()}
                onInput={(e) => setUsername(e.currentTarget.value)}
                class="w-full px-4 py-3 border border-[var(--color-border)] rounded-xl text-sm text-[var(--color-text-primary)] placeholder-[var(--color-text-tertiary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/20 focus:border-[var(--color-accent)] transition-all duration-200"
                placeholder="请输入用户名"
                autocomplete="username"
                disabled={loading()}
              />
            </div>

            {/* 密码 */}
            <div>
              <label for="password" class="block text-[13px] font-medium text-[var(--color-text-secondary)] mb-1.5">
                密码
              </label>
              <input
                id="password"
                type="password"
                value={password()}
                onInput={(e) => setPassword(e.currentTarget.value)}
                class="w-full px-4 py-3 border border-[var(--color-border)] rounded-xl text-sm text-[var(--color-text-primary)] placeholder-[var(--color-text-tertiary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/20 focus:border-[var(--color-accent)] transition-all duration-200"
                placeholder="请输入密码"
                autocomplete="current-password"
                disabled={loading()}
              />
            </div>

            {/* 登录按钮 */}
            <button
              type="submit"
              disabled={loading()}
              class="w-full py-3 px-4 bg-[var(--color-accent)] text-white text-sm font-medium rounded-xl hover:bg-[var(--color-accent-hover)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed transition-all duration-200"
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
