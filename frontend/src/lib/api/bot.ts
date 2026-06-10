import { api } from './client'
import type { ApiResponse, BotSummary } from './types'

// ── Grid Bot ──────────────────────────────────────────────

export async function createGridBot(params: {
  symbol: string
  exchange: string
  grid_count?: number
  upper_price?: number
  lower_price?: number
  grid_profit_pct?: number
  quantity_per_grid?: number
  leverage?: number
  name?: string
  paper_mode?: boolean
}): Promise<ApiResponse<{ id: string }>> {
  return api.post('/grid/create', params)
}

export async function startGridBot(id: string): Promise<ApiResponse<{ started: boolean }>> {
  return api.post(`/grid/${id}/start`)
}

// ── Auto Bot ──────────────────────────────────────────────

export async function createAutoBot(params: {
  symbol: string
  exchange: string
  market_type?: string
  leverage?: number
  name?: string
  paper_mode?: boolean
}): Promise<ApiResponse<{ id: string }>> {
  return api.post('/auto/create', params)
}

export async function startAutoBot(id: string): Promise<ApiResponse<{ started: boolean }>> {
  return api.post(`/auto/${id}/start`)
}

// ── Bot discovery ─────────────────────────────────────────

export async function findActiveBot(): Promise<BotSummary | null> {
  try {
    const [gridRes, autoRes] = await Promise.all([
      api.get<{ items: Array<{ id: string; status: string }>; total: number }>('/grid/list'),
      api.get<{ items: Array<{ id: string; status: string }>; total: number }>('/auto/list'),
    ])
    if (autoRes.success && autoRes.data?.items?.length) {
      const bot = autoRes.data.items.find(b => b.status === 'running') || autoRes.data.items[0]
      return { id: bot.id, bot_type: 'auto' }
    }
    if (gridRes.success && gridRes.data?.items?.length) {
      const bot = gridRes.data.items.find(b => b.status === 'running') || gridRes.data.items[0]
      return { id: bot.id, bot_type: 'grid' }
    }
  } catch {
    // ignore
  }
  return null
}
