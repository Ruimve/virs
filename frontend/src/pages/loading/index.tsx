import { type Component, createSignal, onMount, Show } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import { fetchUser, isLoggedIn } from '../../lib/auth'
import { findActiveBot } from '../../lib/api'
import { loadWizardState } from '../../lib/wizard'

const Loading: Component = () => {
  const navigate = useNavigate()
  const [progress, setProgress] = createSignal(0)
  const [statusText, setStatusText] = createSignal('Initializing...')

  onMount(async () => {
    // Fast path: no token → login immediately
    if (!isLoggedIn()) {
      navigate('/login', { replace: true })
      return
    }

    // Has token — show brief loading while checking
    setProgress(20)
    setStatusText('Checking authentication...')

    const loggedIn = await fetchUser()
    if (!loggedIn) {
      navigate('/login', { replace: true })
      return
    }

    setProgress(50)
    setStatusText('Loading settings...')

    loadWizardState()

    setProgress(70)
    setStatusText('Finding bot...')

    const bot = await findActiveBot()
    if (bot) {
      setProgress(90)
      setStatusText('Routing...')
      await delay(200)
      if (bot.bot_type === 'auto') {
        navigate(`/trade/auto/${bot.id}`, { replace: true })
      } else {
        navigate(`/trade/grid/${bot.id}`, { replace: true })
      }
    } else {
      setProgress(90)
      setStatusText('Routing...')
      await delay(200)
      navigate('/setup/bot-type', { replace: true })
    }
  })

  return (
    <div class="min-h-screen bg-[#0a0a0f] flex flex-col items-center justify-center relative overflow-hidden">
      {/* Ambient glow */}
      <div class="absolute inset-0 overflow-hidden">
        <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      {/* Logo */}
      <div class="relative mb-16">
        <div class="w-20 h-20 rounded-2xl bg-gradient-to-br from-indigo-500/20 to-violet-500/20 border border-indigo-500/20 flex items-center justify-center backdrop-blur-sm">
          <span class="text-3xl font-extralight tracking-[0.3em] text-white/90 select-none">V</span>
        </div>
      </div>

      <h1 class="text-2xl font-extralight tracking-[0.4em] text-white/80 mb-2 select-none">VIRS</h1>
      <p class="text-[11px] tracking-[0.25em] text-white/25 mb-16 select-none">QUANTITATIVE TRADING</p>

      {/* Progress */}
      <Show when={progress() > 0}>
        <div class="w-64 relative">
          <div class="h-[2px] bg-white/[0.06] rounded-full overflow-hidden">
            <div
              class="h-full bg-gradient-to-r from-indigo-400 to-violet-400 rounded-full transition-all duration-500 ease-out"
              style={{ width: `${progress()}%` }}
            />
          </div>
          <div class="flex justify-between items-center mt-4">
            <span class="text-[11px] text-white/20 tracking-wider">{statusText()}</span>
            <span class="text-[11px] text-white/30 font-mono tabular-nums">{progress()}%</span>
          </div>
        </div>
      </Show>
    </div>
  )
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

export default Loading
