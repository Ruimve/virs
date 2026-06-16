import { api } from './client'
import type { ApiResponse, HealthStatus, PaperModeStatus } from './types'

export function checkHealth(): Promise<ApiResponse<HealthStatus>> {
  return api.get<HealthStatus>('/health')
}

export function getPaperStatus(): Promise<ApiResponse<PaperModeStatus>> {
  return api.get<PaperModeStatus>('/system/paper/status')
}

export function enablePaperMode(): Promise<ApiResponse<null>> {
  return api.post<null>('/system/paper/enable')
}

export function disablePaperMode(): Promise<ApiResponse<null>> {
  return api.post<null>('/system/paper/disable')
}
