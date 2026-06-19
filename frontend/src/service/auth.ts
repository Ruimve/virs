import { api, setToken, removeToken } from './client'
import type { ApiResponse, LoginResponse, UserInfo } from './types'

export async function login(
  username: string,
  password: string,
): Promise<ApiResponse<LoginResponse>> {
  const result = await api.post<LoginResponse>('/user/login', { username, password })
  if (result.success && result.data) {
    setToken(result.data.token)
  }
  return result
}

export function logout(): void {
  removeToken()
  window.location.href = '/login'
}

export function getUserInfo(): Promise<ApiResponse<UserInfo>> {
  return api.get<UserInfo>('/user/info')
}
