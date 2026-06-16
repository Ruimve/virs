import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import type { MarketType } from '@/lib/market-context'

// ── 向导步骤定义 ──
export const WizardStep = {
  Loading: 0,
  Login: 1,
  SelectBotType: 2,
  ConfigureLlm: 3,
  SelectExchange: 4,
  ConfigureParams: 5,
  ReviewLaunch: 6,
  Trading: 7,
} as const

export type WizardStepValue = (typeof WizardStep)[keyof typeof WizardStep]

// ── 向导状态 ──
export interface WizardState {
  current_step: WizardStepValue
  exchange: string
  market_type: MarketType
  bot_type: 'grid' | 'auto' | ''
  paper_mode: boolean
  llm_provider: string
  llm_model: string
  bot_params: Record<string, unknown>
  bot_id: string
}

// ── Sensitive credentials (memory only) ──
export interface WizardCredentials {
  exchange_api_key: string
  exchange_api_secret: string
  exchange_passphrase: string
  llm_api_key: string
}

const DEFAULT_STATE: WizardState = {
  current_step: WizardStep.Loading,
  exchange: '',
  market_type: 'perpetual',
  bot_type: '',
  paper_mode: true,
  llm_provider: '',
  llm_model: '',
  bot_params: {},
  bot_id: '',
}

const DEFAULT_CREDENTIALS: WizardCredentials = {
  exchange_api_key: '',
  exchange_api_secret: '',
  exchange_passphrase: '',
  llm_api_key: '',
}

// ── Module-level state with subscriber pattern ──
let _wizardState: WizardState = { ...DEFAULT_STATE }
let _wizardCredentials: WizardCredentials = { ...DEFAULT_CREDENTIALS }
const _listeners = new Set<() => void>()

function notify() {
  _listeners.forEach(l => l())
}

export function subscribe(listener: () => void) {
  _listeners.add(listener)
  return () => {
    _listeners.delete(listener)
  }
}

// ── Getters ──
export function getWizardState(): WizardState {
  return _wizardState
}

export function getWizardCredentials(): WizardCredentials {
  return _wizardCredentials
}

export function getCurrentStep(): WizardStepValue {
  return _wizardState.current_step
}

// ── 更新向导状态 ──
export function updateWizard(partial: Partial<WizardState>) {
  _wizardState = { ..._wizardState, ...partial }
  notify()
}

// ── 更新凭证（仅内存） ──
export function updateCredentials(partial: Partial<WizardCredentials>) {
  _wizardCredentials = { ..._wizardCredentials, ...partial }
  notify()
}

// ── 推进到下一步 ──
export function advanceStep(step?: WizardStepValue) {
  _wizardState = {
    ..._wizardState,
    current_step: step ?? ((_wizardState.current_step + 1) as WizardStepValue),
  }
  notify()
}

// ── 重置向导 ──
export function resetWizard() {
  _wizardState = { ...DEFAULT_STATE }
  _wizardCredentials = { ...DEFAULT_CREDENTIALS }
  notify()
}

// ── 向导步骤守卫：直接访问非首页时跳转首页 ──
export function useWizardGuard(requiredStep: WizardStepValue) {
  const navigate = useNavigate()
  useEffect(() => {
    if (_wizardState.current_step < requiredStep) {
      navigate('/setup/bot-type', { replace: true })
    }
  }, [requiredStep, navigate])
}

// React hook
export function useWizardState() {
  const [, forceUpdate] = useState(0)
  useEffect(() => {
    return subscribe(() => forceUpdate(v => v + 1))
  }, [])
  return {
    wizardState: getWizardState(),
    wizardCredentials: getWizardCredentials(),
    currentStep: getCurrentStep(),
    updateWizard,
    updateCredentials,
    advanceStep,
    resetWizard,
  }
}
