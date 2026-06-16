import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { Wizard } from '../components/Wizard'
import { updateWizard, advanceStep, WizardStep, getWizardState } from '../components/Wizard/wizard'
import { findActiveBot } from '../../../service'

const BOT_TYPES = [
  {
    id: 'grid' as const,
    name: 'Grid Bot',
    desc: 'Profit from market volatility by placing orders at regular price intervals. Best for ranging markets.',
    features: ['Grid levels', 'Auto buy/sell', 'Range-bound profit'],
    color: 'from-indigo-500/20 to-blue-500/20',
    border: 'border-indigo-500/30',
  },
  {
    id: 'auto' as const,
    name: 'Auto Bot',
    desc: 'AI-driven fully automated trading. Analyzes market conditions and executes trades autonomously.',
    features: ['AI analysis', 'Auto execution', 'Multi-market'],
    color: 'from-violet-500/20 to-purple-500/20',
    border: 'border-violet-500/30',
  },
]

function SelectBotType() {
  const navigate = useNavigate()
  const wizard = getWizardState()
  const [selected, setSelected] = useState<'grid' | 'auto' | ''>(wizard.bot_type || '')
  const [existingBot, setExistingBot] = useState<{ id: string; bot_type: string } | null>(null)

  useEffect(() => {
    findActiveBot().then(bot => { if (bot) setExistingBot(bot) })
  }, [])

  const canContinue = selected.length > 0 && !existingBot

  const handleContinue = () => {
    updateWizard({ bot_type: selected })
    advanceStep(WizardStep.ConfigureLlm)
    navigate('/setup/llm', { replace: true })
  }

  return (
    <Wizard
      step={WizardStep.SelectBotType}
      title="Choose Strategy"
      subtitle="Select the trading strategy that fits your goals"
      actions={
        <button
          onClick={handleContinue}
          disabled={!canContinue}
          className="w-full sm:w-auto sm:px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
        >
          Continue
        </button>
      }
    >
      {existingBot ? (
        <div className="p-5 rounded-xl border border-amber-500/20 bg-amber-500/5 mb-8">
          <div className="flex items-start gap-3">
            <svg className="w-5 h-5 text-amber-400 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="2">
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <div>
              <p className="text-sm font-medium text-on-base mb-1">已有机器人</p>
              <p className="text-xs text-on-surface-tertiary mb-3">每个账号只能创建一个机器人，请先删除已有机器人。</p>
              <button
                onClick={() => navigate(existingBot.bot_type === 'auto' ? `/trade/auto/${existingBot.id}` : `/trade/grid/${existingBot.id}`, { replace: true })}
                className="px-4 py-2 text-xs font-medium bg-indigo-500/10 border border-indigo-500/20 text-indigo-400 hover:bg-indigo-500/20 rounded-lg transition-colors"
              >
                查看已有机器人
              </button>
            </div>
          </div>
        </div>
      ) : (
        <div className="space-y-4 mb-8">
          {BOT_TYPES.map((bot) => {
            const isSelected = selected === bot.id
            return (
              <button
                key={bot.id}
                onClick={() => setSelected(bot.id)}
                className={`group w-full p-4 md:p-5 rounded-xl border text-left transition-all duration-200 ${
                  isSelected
                    ? `bg-gradient-to-br ${bot.color} ${bot.border} ring-1 ring-line-emphasis`
                    : 'bg-surface-1 border-line-default hover:bg-surface-2 hover:border-line-emphasis'
                }`}
              >
                <div className="flex items-start gap-4">
                  <div className={`shrink-0 ${isSelected ? 'text-indigo-400' : 'text-on-surface-faint'}`}>
                    {bot.id === 'grid' ? (
                      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="1.2">
                        <path strokeLinecap="round" strokeLinejoin="round" d="M3.75 6A2.25 2.25 0 016 3.75h2.25A2.25 2.25 0 0110.5 6v2.25a2.25 2.25 0 01-2.25 2.25H6a2.25 2.25 0 01-2.25-2.25V6zM3.75 15.75A2.25 2.25 0 016 13.5h2.25a2.25 2.25 0 012.25 2.25V18a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 18v-2.25zM13.5 6a2.25 2.25 0 012.25-2.25H18A2.25 2.25 0 0120.25 6v2.25A2.25 2.25 0 0118 10.5h-2.25a2.25 2.25 0 01-2.25-2.25V6zM13.5 15.75a2.25 2.25 0 012.25-2.25H18a2.25 2.25 0 012.25 2.25V18A2.25 2.25 0 0118 20.25h-2.25a2.25 2.25 0 01-2.25-18v-2.25z" />
                      </svg>
                    ) : (
                      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="1.2">
                        <path strokeLinecap="round" strokeLinejoin="round" d="M15.59 14.37a6 6 0 01-5.84 7.38v-4.8m5.84-2.58a14.98 14.98 0 006.16-12.12A14.98 14.98 0 009.631 8.41m5.96 5.96a14.926 14.926 0 01-5.841 2.58m-.119-8.54a6 6 0 00-7.381 5.84h4.8m2.581-5.84a14.927 14.927 0 00-2.58 5.84m2.699 2.7c-.103.021-.207.041-.311.06a15.09 15.09 0 01-2.448-2.448 14.9 14.9 0 01.06-.312m-2.24 2.39a4.493 4.493 0 00-1.757 4.306 4.493 4.493 0 004.306-1.758M16.5 9a1.5 1.5 0 11-3 0 1.5 1.5 0 013 0z" />
                      </svg>
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className={`text-base font-medium ${isSelected ? 'text-on-base' : 'text-on-surface-secondary'}`}>
                      {bot.name}
                    </p>
                    <p className="text-[13px] text-on-surface-tertiary mt-1 leading-relaxed">{bot.desc}</p>
                    <div className="flex flex-wrap gap-2 mt-3">
                      {bot.features.map((f) => (
                        <span key={f} className={`px-2 py-0.5 rounded-md text-[11px] ${
                          isSelected ? 'bg-surface-3 text-on-surface-tertiary' : 'bg-surface-1 text-on-surface-faint'
                        }`}>
                          {f}
                        </span>
                      ))}
                    </div>
                  </div>
                  {isSelected && (
                    <div className="shrink-0 w-5 h-5 rounded-full bg-indigo-500 flex items-center justify-center">
                      <svg className="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="3">
                        <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                      </svg>
                    </div>
                  )}
                </div>
              </button>
            )
          })}
        </div>
      )}
    </Wizard>
  )
}

export default SelectBotType
