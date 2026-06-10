import { type Component, createSignal, Show } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import { login } from '../../lib/api'
import { advanceStep, loadWizardState, getCurrentStep, WizardStep } from '../../lib/wizard'

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
      setError('Please enter username and password')
      return
    }

    setLoading(true)
    try {
      const result = await login(username().trim(), password())
      if (result.success) {
        // Load wizard state to determine if setup is needed
        loadWizardState()
        if (getCurrentStep() === WizardStep.Trading) {
          // Already set up → service check
          navigate('/check', { replace: true })
        } else {
          // Not set up → wizard
          advanceStep(WizardStep.SelectExchange)
          navigate('/setup/exchange', { replace: true })
        }
      } else {
        setError(result.error || 'Login failed')
      }
    } catch {
      setError('Network error, please try again')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div class="min-h-screen bg-[#0a0a0f] flex items-center justify-center relative overflow-hidden">
      {/* Background */}
      <div class="absolute inset-0 overflow-hidden">
        <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      <div class="w-full max-w-sm px-6 relative">
        {/* Logo */}
        <div class="text-center mb-12">
          <div class="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-gradient-to-br from-indigo-500/20 to-violet-500/20 border border-indigo-500/20 mb-6">
            <span class="text-2xl font-extralight tracking-[0.3em] text-white/90">V</span>
          </div>
          <h1 class="text-2xl font-extralight tracking-[0.4em] text-white/80 mb-1">VIRS</h1>
          <p class="text-[11px] tracking-[0.25em] text-white/25">QUANTITATIVE TRADING</p>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} class="space-y-5">
          <Show when={error()}>
            {(errorMsg) => (
              <div class="flex items-center gap-2 p-3 bg-red-500/10 border border-red-500/20 rounded-xl text-sm text-red-400">
                <svg class="w-4 h-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <span>{errorMsg()}</span>
              </div>
            )}
          </Show>

          <div>
            <input
              type="text"
              value={username()}
              onInput={(e) => setUsername(e.currentTarget.value)}
              class="w-full px-4 py-3 bg-white/[0.04] border border-white/[0.08] rounded-xl text-sm text-white/90 placeholder-white/20 focus:outline-none focus:border-indigo-500/40 focus:bg-white/[0.06] transition-all duration-200"
              placeholder="Username"
              autocomplete="username"
              disabled={loading()}
            />
          </div>

          <div>
            <input
              type="password"
              value={password()}
              onInput={(e) => setPassword(e.currentTarget.value)}
              class="w-full px-4 py-3 bg-white/[0.04] border border-white/[0.08] rounded-xl text-sm text-white/90 placeholder-white/20 focus:outline-none focus:border-indigo-500/40 focus:bg-white/[0.06] transition-all duration-200"
              placeholder="Password"
              autocomplete="current-password"
              disabled={loading()}
            />
          </div>

          <button
            type="submit"
            disabled={loading()}
            class="w-full py-3 px-4 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl focus:outline-none focus:ring-2 focus:ring-indigo-500/30 focus:ring-offset-2 focus:ring-offset-[#0a0a0f] disabled:opacity-40 disabled:cursor-not-allowed transition-all duration-200"
          >
            <Show when={loading()} fallback="Sign in">
              <span class="flex items-center justify-center gap-2">
                <svg class="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                </svg>
                Signing in...
              </span>
            </Show>
          </button>
        </form>
      </div>
    </div>
  )
}

export default Login
