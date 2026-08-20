import { api } from './client';
import type { ApiResponse, HealthStatus, PaperModeStatus, SystemInfo } from './types';

export function checkHealth(params?: {}, init?: RequestInit): Promise<ApiResponse<HealthStatus>> {
  return api.get<HealthStatus>('/health', params, init);
}

export function getPaperStatus(
  params?: {},
  init?: RequestInit,
): Promise<ApiResponse<PaperModeStatus>> {
  return api.get<PaperModeStatus>('/system/paper/status', params, init);
}

export function getSystemInfo(params?: {}, init?: RequestInit): Promise<ApiResponse<SystemInfo>> {
  return api.get<SystemInfo>('/system/info', params, init);
}
