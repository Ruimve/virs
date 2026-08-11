import { api, setToken } from './client';
import type { ApiResponse, LoginResponse, UserInfo } from './types';

export async function login(
  username: string,
  password: string,
  init?: RequestInit,
): Promise<ApiResponse<LoginResponse>> {
  const result = await api.post<LoginResponse>('/user/login', { username, password }, init);
  if (result.success && result.data) {
    setToken(result.data.token);
  }
  return result;
}

export function getUserInfo(init?: RequestInit): Promise<ApiResponse<UserInfo>> {
  return api.get<UserInfo>('/user/info', init);
}
