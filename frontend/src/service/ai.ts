import { api } from './client';
import type { ApiResponse, Plugin, AiGenerateResult } from './types';

export async function fetchPlugins(): Promise<ApiResponse<Plugin[]>> {
  return api.get<Plugin[]>('/plugins');
}

export async function validateScript(
  code: string,
): Promise<ApiResponse<{ valid: boolean; error?: string }>> {
  return api.post('/strategy/validate-script', { code });
}

export async function getAiStatus(): Promise<
  ApiResponse<{ configured: boolean; providers: string[] }>
> {
  return api.get('/ai/status');
}

export async function generateStrategy(
  prompt: string,
  provider?: string,
  model?: string,
): Promise<ApiResponse<AiGenerateResult>> {
  return api.post('/ai/generate', { prompt, provider, model });
}
