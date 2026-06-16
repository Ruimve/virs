import { api } from './client'
import type {
  ApiResponse,
  BotSummary,
  AutoBotDetail,
  GridBotDetail,
  GridTradesResponse,
  AnalysisLog,
} from './types'

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
  market_type?: string
}): Promise<ApiResponse<{ id: string }>> {
  return api.post('/grid/create', params)
}

export async function startGridBot(id: string): Promise<ApiResponse<{ started: boolean }>> {
  return api.post(`/grid/${id}/start`)
}

export async function stopGridBot(id: string): Promise<ApiResponse<{ stopped: boolean }>> {
  return api.post(`/grid/${id}/stop`)
}

export async function deleteGridBot(id: string): Promise<ApiResponse<null>> {
  return api.del(`/grid/${id}/delete`)
}

export async function getGridBotDetail(id: string): Promise<ApiResponse<GridBotDetail>> {
  return api.get<GridBotDetail>(`/grid/${id}`)
}

export async function getGridTrades(id: string): Promise<ApiResponse<GridTradesResponse>> {
  return api.get<GridTradesResponse>(`/grid/${id}/trades`)
}

export async function getGridAnalysisLogs(botId: string): Promise<ApiResponse<{ items: AnalysisLog[] }>> {
  return api.get<{ items: AnalysisLog[] }>(`/grid/analysis-logs?bot_id=${botId}`)
}

// ── Auto Bot ──────────────────────────────────────────────

export async function createAutoBot(params: {
  symbol: string
  exchange: string
  market_type?: string
  leverage?: number
  decide_interval_secs?: number
  name?: string
  paper_mode?: boolean
}): Promise<ApiResponse<{ id: string }>> {
  return api.post('/auto/create', params)
}

export async function startAutoBot(id: string): Promise<ApiResponse<{ started: boolean }>> {
  return api.post(`/auto/${id}/start`)
}

export async function stopAutoBot(id: string): Promise<ApiResponse<{ stopped: boolean }>> {
  return api.post(`/auto/${id}/stop`)
}

export async function deleteAutoBot(id: string): Promise<ApiResponse<null>> {
  return api.del(`/auto/${id}/delete`)
}

export async function getAutoBotDetail(id: string): Promise<ApiResponse<AutoBotDetail>> {
  return api.get<AutoBotDetail>(`/auto/${id}`)
}

export async function getAutoAnalysisLogs(botId: string): Promise<ApiResponse<{ logs: AnalysisLog[] }>> {
  return api.get<{ logs: AnalysisLog[] }>(`/auto/analysis-logs?bot_id=${botId}`)
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
