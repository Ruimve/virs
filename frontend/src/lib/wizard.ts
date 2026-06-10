import { createSignal } from 'solid-js'
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
// Note: API keys/secrets are NOT persisted to localStorage for security.
// They are held in memory only and must be re-entered after page refresh.
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

const [wizardState, setWizardState] = createSignal<WizardState>(loadFromStorage())
const [wizardCredentials, setWizardCredentials] = createSignal<WizardCredentials>({ ...DEFAULT_CREDENTIALS })

export function getWizardState() {
  return wizardState
}

export function getWizardCredentials() {
  return wizardCredentials
}

export function getCurrentStep(): WizardStepValue {
  return wizardState().current_step
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
    // Only persist non-sensitive state
    const state = wizardState()
    localStorage.setItem(WIZARD_STORAGE_KEY, JSON.stringify(state))
  } catch {
    // ignore
  }
}

// ── 更新向导状态 ──
export function updateWizard(partial: Partial<WizardState>) {
  setWizardState((prev) => ({ ...prev, ...partial }))
  saveToStorage()
}

// ── 更新凭证（仅内存） ──
export function updateCredentials(partial: Partial<WizardCredentials>) {
  setWizardCredentials((prev) => ({ ...prev, ...partial }))
}

// ── 推进到下一步 ──
export function advanceStep(step?: WizardStepValue) {
  setWizardState((prev) => ({
    ...prev,
    current_step: step ?? (prev.current_step + 1) as WizardStepValue,
  }))
  saveToStorage()
}

// ── 从存储恢复向导状态 ──
export function loadWizardState(): boolean {
  const stored = loadFromStorage()
  setWizardState(stored)
  return stored.current_step > WizardStep.Loading
}

// ── 重置向导 ──
export function resetWizard() {
  setWizardState({ ...DEFAULT_STATE })
  setWizardCredentials({ ...DEFAULT_CREDENTIALS })
  try {
    localStorage.removeItem(WIZARD_STORAGE_KEY)
  } catch {
    // ignore
  }
}

// ── 根据向导状态决定初始路由 ──
export function resolveInitialRoute(): string {
  const step = wizardState().current_step

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
