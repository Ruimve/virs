import { api } from './client';
import type { ApiResponse, HealthStatus, PaperModeStatus, SystemInfo } from './types';

export function checkHealth(init?: RequestInit): Promise<ApiResponse<HealthStatus>> {
  return api.get<HealthStatus>('/health', init);
}

export function getPaperStatus(init?: RequestInit): Promise<ApiResponse<PaperModeStatus>> {
  return api.get<PaperModeStatus>('/system/paper/status', init);
}

export function getSystemInfo(init?: RequestInit): Promise<ApiResponse<SystemInfo>> {
  return api.get<SystemInfo>('/system/info', init);
}
