import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { fetchUser, isLoggedIn } from '../../lib/auth'
import { findActiveBot } from '../../service'

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function Loading() {
  const navigate = useNavigate()
  const [progress, setProgress] = useState(0)
  const [statusText, setStatusText] = useState('Initializing...')

  useEffect(() => {
    if (!isLoggedIn()) {
      navigate('/login', { replace: true })
      return
    }

    setProgress(20)
    setStatusText('Checking authentication...')

    fetchUser().then((loggedIn) => {
      if (!loggedIn) {
        navigate('/login', { replace: true })
        return
      }

      setProgress(50)
      setStatusText('Loading settings...')

      setProgress(70)
      setStatusText('Finding bot...')

      findActiveBot().then((bot) => {
        setProgress(90)
        setStatusText('Routing...')
        delay(200).then(() => {
          if (bot) {
            if (bot.bot_type === 'auto') {
              navigate(`/trade/auto/${bot.id}/bot`, { replace: true })
            } else {
              navigate(`/trade/grid/${bot.id}/bot`, { replace: true })
            }
          } else {
            navigate('/setup/bot-type', { replace: true })
          }
        })
      })
    })
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="min-h-screen bg-base flex flex-col items-center justify-center relative overflow-hidden">
      <div className="absolute inset-0 overflow-hidden">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      <div className="relative mb-16">
        <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-indigo-500/20 to-violet-500/20 border border-indigo-500/20 flex items-center justify-center backdrop-blur-sm">
          <span className="text-3xl font-extralight tracking-[0.3em] text-on-base select-none">
            V
          </span>
        </div>
      </div>

      <h1 className="text-2xl font-extralight tracking-[0.4em] text-on-surface mb-2 select-none">
        VIRS
      </h1>
      <p className="text-[11px] tracking-[0.25em] text-on-surface-muted mb-16 select-none">
        QUANTITATIVE TRADING
      </p>

      {progress > 0 && (
        <div className="w-64 relative">
          <div className="h-[2px] bg-line-default rounded-full overflow-hidden">
            <div
              className="h-full bg-gradient-to-r from-indigo-400 to-violet-400 rounded-full transition-all duration-500 ease-out"
              style={{ width: `${progress}%` }}
            />
          </div>
          <div className="flex justify-between items-center mt-4">
            <span className="text-[11px] text-on-surface-faint tracking-wider">{statusText}</span>
            <span className="text-[11px] text-on-surface-tertiary font-mono tabular-nums">
              {progress}%
            </span>
          </div>
        </div>
      )}
    </div>
  )
}

export default Loading
