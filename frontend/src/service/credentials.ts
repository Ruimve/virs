import { api } from './client';
import type {
  ApiResponse,
  BalanceInfo,
  DeepSeekModel,
  PermissionItem,
  PositionModeResult,
} from './types';

export async function saveCredential(params: {
  exchange: string;
  api_key: string;
  api_secret: string;
  passphrase?: string;
  label?: string;
}): Promise<ApiResponse<{ id: string }>> {
  return api.post('/credentials/save', params);
}

export async function saveAiCredential(params: {
  provider: string;
  api_key: string;
  model?: string;
  label?: string;
  is_default?: boolean;
}): Promise<ApiResponse<{ id: string }>> {
  return api.post('/ai-credentials/save', params);
}

export async function testCredential(): Promise<
  ApiResponse<{ connected: boolean; message?: string }>
> {
  return api.get('/credentials/test');
}

export async function checkPermissions(): Promise<ApiResponse<{ permissions: PermissionItem[] }>> {
  return api.get('/credentials/check-permissions');
}

export async function fetchPositionMode(): Promise<ApiResponse<PositionModeResult>> {
  return api.get('/credentials/position-mode');
}

export async function fetchCredentialStatus(): Promise<
  ApiResponse<{ connected: boolean; exchange?: string }>
> {
  return api.get('/credentials/status');
}

export async function fetchAiModels(): Promise<ApiResponse<{ models: DeepSeekModel[] }>> {
  return api.get('/ai-credentials/models');
}

export async function fetchAiBalance(): Promise<ApiResponse<{ balances: BalanceInfo[] }>> {
  return api.get('/ai-credentials/balance');
}

export async function testAiCredential(): Promise<
  ApiResponse<{ connected: boolean; message: string }>
> {
  return api.get('/ai-credentials/test');
}
