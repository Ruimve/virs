import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import WizardLayout from '../../../components/WizardLayout'
import { updateWizard, advanceStep, WizardStep, getWizardState } from '../../../lib/wizard'
import type { MarketType } from '../../../lib/market-context'

const MARKET_TYPES: Array<{ id: MarketType; label: string; desc: string }> = [
  { id: 'perpetual', label: 'Perpetual', desc: 'USDT-M futures' },
  { id: 'spot', label: 'Spot', desc: 'Spot trading' },
]

// Grid bot parameters
const GRID_PARAMS = [
  { key: 'symbol', label: 'Trading Pair', type: 'text' as const, placeholder: 'BTC/USDT', required: true },
  { key: 'upper_price', label: 'Upper Price', type: 'number' as const, placeholder: '70000', required: true },
  { key: 'lower_price', label: 'Lower Price', type: 'number' as const, placeholder: '60000', required: true },
  { key: 'grid_levels', label: 'Grid Levels', type: 'number' as const, placeholder: '10', required: true },
  { key: 'investment', label: 'Investment (USDT)', type: 'number' as const, placeholder: '1000', required: true },
  { key: 'leverage', label: 'Leverage', type: 'number' as const, placeholder: '5', required: true },
]

// Auto bot parameters
const AUTO_PARAMS = [
  { key: 'symbol', label: 'Trading Pair', type: 'text' as const, placeholder: 'BTC/USDT', required: true },
  { key: 'leverage', label: 'Leverage', type: 'number' as const, placeholder: '10', required: true },
  { key: 'max_position', label: 'Max Position %', type: 'number' as const, placeholder: '80', required: false },
]

function ConfigureParams() {
  const navigate = useNavigate()
  const wizard = getWizardState()
  const isGrid = wizard.bot_type === 'grid'
  const params = isGrid ? GRID_PARAMS : AUTO_PARAMS

  const [values, setValues] = useState<Record<string, string>>(
    (wizard.bot_params as Record<string, string>) || {}
  )
  const [selectedMarket, setSelectedMarket] = useState<MarketType>(wizard.market_type || 'perpetual')

  const setValue = (key: string, val: string) => {
    setValues((prev) => ({ ...prev, [key]: val }))
  }

  const canContinue = params.filter((p) => p.required).every((p) => values[p.key]?.trim())

  const handleContinue = () => {
    updateWizard({ bot_params: values, market_type: selectedMarket })
    advanceStep(WizardStep.ReviewLaunch)
    navigate('/setup/review', { replace: true })
  }

  return (
    <WizardLayout
      step={WizardStep.ConfigureParams}
      title={isGrid ? 'Grid Parameters' : 'Auto Trading Parameters'}
      subtitle="Configure the trading parameters for your bot"
      actions={
        <>
          <button
            onClick={() => navigate('/setup/exchange', { replace: true })}
            className="w-full sm:w-auto sm:px-5 py-2.5 text-sm text-on-surface-tertiary hover:text-on-surface-secondary rounded-xl transition-colors duration-200"
          >
            Back
          </button>
          <button
            onClick={handleContinue}
            disabled={!canContinue}
            className="w-full sm:w-auto sm:px-6 py-2.5 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200"
          >
            Continue
          </button>
        </>
      }
    >
      <div className="space-y-4">
        {/* Market type toggle */}
        <div>
          <p className="text-[11px] tracking-[0.15em] text-on-surface-muted uppercase mb-3">Market Type</p>
          <div className="grid grid-cols-2 gap-3">
            {MARKET_TYPES.map((mt) => (
              <button
                key={mt.id}
                onClick={() => setSelectedMarket(mt.id)}
                className={`flex-1 p-3 rounded-xl border text-center transition-all duration-200 ${
                  selectedMarket === mt.id
                    ? 'bg-indigo-500/10 border-indigo-500/30 text-on-base'
                    : 'bg-surface-1 border-line-default text-on-surface-tertiary hover:bg-surface-2'
                }`}
              >
                <p className="text-sm font-medium">{mt.label}</p>
                <p className="text-[11px] text-on-surface-muted mt-0.5">{mt.desc}</p>
              </button>
            ))}
          </div>
        </div>

        {params.map((param) => (
          <div key={param.key}>
            <label className="block text-[11px] tracking-[0.15em] text-on-surface-muted uppercase mb-2">
              {param.label}
              {param.required && <span className="text-indigo-400/60 ml-1">*</span>}
            </label>
            <input
              type={param.type}
              value={values[param.key] || ''}
              onInput={(e) => setValue(param.key, e.currentTarget.value)}
              className="w-full px-4 py-2.5 bg-surface-2 border border-line-strong rounded-lg text-sm text-on-base placeholder-placeholder focus:outline-none focus:border-indigo-500/40 transition-all duration-200"
              placeholder={param.placeholder}
            />
          </div>
        ))}
      </div>
    </WizardLayout>
  )
}

export default ConfigureParams
