import { api, setToken } from './client';
import type { ApiResponse, LoginResponse, UserInfo } from './types';

export async function login(
  params?: {},
  bodyParams?: { username: string; password: string },
  init?: RequestInit,
): Promise<ApiResponse<LoginResponse>> {
  const result = await api.post<LoginResponse>('/user/login', params, bodyParams, init);
  if (result.success && result.data) {
    setToken(result.data.token);
  }
  return result;
}

export function getUserInfo(params?: {}, init?: RequestInit): Promise<ApiResponse<UserInfo>> {
  return api.get<UserInfo>('/user/info', params, init);
}
