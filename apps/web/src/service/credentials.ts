import { api } from './client';
import type {
  ApiResponse,
  BalanceInfo,
  DeepSeekModel,
  PermissionItem,
  PositionModeResult,
} from './types';

export async function saveCredential(
  params?: {},
  bodyParams?: {
    exchange: string;
    api_key: string;
    api_secret: string;
    passphrase?: string;
    label: string;
  },
  init?: RequestInit,
): Promise<ApiResponse<{ id: string }>> {
  return api.post('/credentials/save', params, bodyParams, init);
}

export async function saveAiCredential(
  params?: {},
  bodyParams?: {
    provider: string;
    api_key: string;
    model?: string;
    label?: string;
    is_default?: boolean;
  },
  init?: RequestInit,
): Promise<ApiResponse<{ id: string }>> {
  return api.post('/ai-credentials/save', params, bodyParams, init);
}

export async function testCredential(
  params?: {},
  init?: RequestInit,
): Promise<ApiResponse<{ connected: boolean; message?: string }>> {
  return api.get('/credentials/test', params, init);
}

export async function checkPermissions(
  params?: {},
  init?: RequestInit,
): Promise<ApiResponse<{ permissions: PermissionItem[] }>> {
  return api.get('/credentials/check-permissions', params, init);
}

export async function fetchPositionMode(
  params?: {},
  init?: RequestInit,
): Promise<ApiResponse<PositionModeResult>> {
  return api.get('/credentials/position-mode', params, init);
}

export async function fetchCredentialStatus(
  params?: {},
  init?: RequestInit,
): Promise<ApiResponse<{ connected: boolean; exchange?: string }>> {
  return api.get('/credentials/status', params, init);
}

export async function fetchAiModels(
  params?: {},
  init?: RequestInit,
): Promise<ApiResponse<{ models: DeepSeekModel[] }>> {
  return api.get('/ai-credentials/models', params, init);
}

export async function fetchAiBalance(
  params?: {},
  init?: RequestInit,
): Promise<ApiResponse<{ balances: BalanceInfo[] }>> {
  return api.get('/ai-credentials/balance', params, init);
}

export async function testAiCredential(
  params?: {},
  init?: RequestInit,
): Promise<ApiResponse<{ connected: boolean; message: string }>> {
  return api.get('/ai-credentials/test', params, init);
}
