import { api } from './client';
import type { ApiResponse, HealthStatus, PaperModeStatus, SystemInfo } from './types';

export function checkHealth(): Promise<ApiResponse<HealthStatus>> {
  return api.get<HealthStatus>('/health');
}

export function getPaperStatus(): Promise<ApiResponse<PaperModeStatus>> {
  return api.get<PaperModeStatus>('/system/paper/status');
}

export function getSystemInfo(): Promise<ApiResponse<SystemInfo>> {
  return api.get<SystemInfo>('/system/info');
}
