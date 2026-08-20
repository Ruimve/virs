import { api } from './client';
import type { ApiResponse } from './types';

export async function getAiStatus(
  params?: {},
  init?: RequestInit,
): Promise<ApiResponse<{ configured: boolean; providers: string[] }>> {
  return api.get('/ai/status', params, init);
}
