import { useState, useEffect } from 'react'
import type { MarketType } from './market-context'

// ── 向导步骤定义 ──
export const WizardStep = {
  Loading: 0,
  Login: 1,
  SelectBotType: 2,
  ConfigureLlm: 3,
  SelectExchange: 4,
  ConfigureParams: 5,
  ReviewLaunch: 6,
  HealthCheck: 7,
  Trading: 8,
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

// ── Sensitive credentials (memory only, not persisted) ──
export interface WizardCredentials {
  exchange_api_key: string
  exchange_api_secret: string
  exchange_passphrase: string
  llm_api_key: string
}

const WIZARD_STORAGE_KEY = 'virs_wizard_state'

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
let _wizardState: WizardState = loadFromStorage()
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

// ── localStorage 持久化 (non-sensitive data only) ──
function loadFromStorage(): WizardState {
  try {
    const raw = localStorage.getItem(WIZARD_STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<WizardState>
      return { ...DEFAULT_STATE, ...parsed }
    }
  } catch {
    // ignore
  }
  return { ...DEFAULT_STATE }
}

function saveToStorage() {
  try {
    localStorage.setItem(WIZARD_STORAGE_KEY, JSON.stringify(_wizardState))
  } catch {
    // ignore
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
  saveToStorage()
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
  saveToStorage()
  notify()
}

// ── 从存储恢复向导状态 ──
export function loadWizardState(): boolean {
  const stored = loadFromStorage()
  _wizardState = stored
  notify()
  return stored.current_step > WizardStep.Loading
}

// ── 重置向导 ──
export function resetWizard() {
  _wizardState = { ...DEFAULT_STATE }
  _wizardCredentials = { ...DEFAULT_CREDENTIALS }
  try {
    localStorage.removeItem(WIZARD_STORAGE_KEY)
  } catch {
    // ignore
  }
  notify()
}

// ── 根据向导状态决定初始路由 ──
export function resolveInitialRoute(): string {
  const step = _wizardState.current_step

  if (step === WizardStep.Trading) return '/check'

  switch (step) {
    case WizardStep.SelectBotType:
      return '/setup/bot-type'
    case WizardStep.ConfigureLlm:
      return '/setup/llm'
    case WizardStep.SelectExchange:
      return '/setup/exchange'
    case WizardStep.ConfigureParams:
      return '/setup/params'
    case WizardStep.ReviewLaunch:
      return '/setup/review'
    case WizardStep.HealthCheck:
      return '/setup/health'
    default:
      return '/setup/bot-type'
  }
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
    loadWizardState,
    resetWizard,
  }
}
