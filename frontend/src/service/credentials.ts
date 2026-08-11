import { api } from './client';
import type {
  ApiResponse,
  BalanceInfo,
  DeepSeekModel,
  PermissionItem,
  PositionModeResult,
} from './types';

export async function saveCredential(
  params: {
    exchange: string;
    api_key: string;
    api_secret: string;
    passphrase?: string;
    label?: string;
  },
  init?: RequestInit,
): Promise<ApiResponse<{ id: string }>> {
  return api.post('/credentials/save', params, init);
}

export async function saveAiCredential(
  params: {
    provider: string;
    api_key: string;
    model?: string;
    label?: string;
    is_default?: boolean;
  },
  init?: RequestInit,
): Promise<ApiResponse<{ id: string }>> {
  return api.post('/ai-credentials/save', params, init);
}

export async function testCredential(
  init?: RequestInit,
): Promise<ApiResponse<{ connected: boolean; message?: string }>> {
  return api.get('/credentials/test', init);
}

export async function checkPermissions(
  init?: RequestInit,
): Promise<ApiResponse<{ permissions: PermissionItem[] }>> {
  return api.get('/credentials/check-permissions', init);
}

export async function fetchPositionMode(
  init?: RequestInit,
): Promise<ApiResponse<PositionModeResult>> {
  return api.get('/credentials/position-mode', init);
}

export async function fetchCredentialStatus(
  init?: RequestInit,
): Promise<ApiResponse<{ connected: boolean; exchange?: string }>> {
  return api.get('/credentials/status', init);
}

export async function fetchAiModels(
  init?: RequestInit,
): Promise<ApiResponse<{ models: DeepSeekModel[] }>> {
  return api.get('/ai-credentials/models', init);
}

export async function fetchAiBalance(
  init?: RequestInit,
): Promise<ApiResponse<{ balances: BalanceInfo[] }>> {
  return api.get('/ai-credentials/balance', init);
}

export async function testAiCredential(
  init?: RequestInit,
): Promise<ApiResponse<{ connected: boolean; message: string }>> {
  return api.get('/ai-credentials/test', init);
}
