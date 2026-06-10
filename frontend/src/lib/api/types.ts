// ── API response types ──────────────────────────────────────

export interface ApiResponse<T = unknown> {
  success: boolean
  data?: T
  error?: string
  message?: string
}

export interface PaginatedResponse<T> {
  items: T[]
  total: number
  page: number
  page_size: number
  total_pages: number
}

// ── User types ──────────────────────────────────────────────

export interface UserInfo {
  id: string
  username: string
  role: 'admin' | 'user'
  created_at?: string
}

export interface LoginResponse {
  token: string
  user: UserInfo
}

// ── Plugin types ────────────────────────────────────────────

export interface PluginParam {
  name: string
  label: string
  param_type: 'int' | 'float'
  default: number
  min?: number
  max?: number
  step?: number
}

export interface Plugin {
  name: string
  description: string
  category: string
  params: PluginParam[]
  required_timeframes?: string[]
}

// ── AI types ────────────────────────────────────────────────

export interface AiGeneratedParam {
  name: string
  label: string
  default: number
  min: number | null
  max: number | null
  step: number | null
}

export interface AiGenerateResult {
  code: string
  name: string
  description: string
  params: AiGeneratedParam[]
  provider: string
  model: string
}

// ── Credentials types ───────────────────────────────────────

export interface PermissionItem {
  name: string
  label: string
  status: 'ok' | 'warn' | 'error'
  detail: string
}

export interface AccountInfo {
  perpetual_usdt: number | null
  spot_usdt: number | null
}

// ── Bot types ───────────────────────────────────────────────

export interface BotSummary {
  id: string
  bot_type: 'grid' | 'auto'
}
