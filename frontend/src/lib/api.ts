// API 响应基础类型
export interface ApiResponse<T = unknown> {
  success: boolean
  data?: T
  error?: string
  message?: string
}

// 分页响应类型
export interface PaginatedResponse<T> {
  items: T[]
  total: number
  page: number
  page_size: number
  total_pages: number
}

// 用户信息类型
export interface UserInfo {
  id: number
  username: string
  role: 'admin' | 'user'
  created_at?: string
}

// 登录响应类型
interface LoginResponse {
  token: string
  user: UserInfo
}

const TOKEN_KEY = 'qd_token'

function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY)
}

function setToken(token: string): void {
  localStorage.setItem(TOKEN_KEY, token)
}

function removeToken(): void {
  localStorage.removeItem(TOKEN_KEY)
}

async function request<T>(
  method: string,
  url: string,
  data?: unknown
): Promise<ApiResponse<T>> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  }

  const token = getToken()
  if (token) {
    headers['Authorization'] = `Bearer ${token}`
  }

  const config: RequestInit = {
    method,
    headers,
  }

  if (data !== undefined && method !== 'GET') {
    config.body = JSON.stringify(data)
  }

  const response = await fetch(`/api${url}`, config)

  // 401 未授权 - 清除 token 并跳转登录页
  if (response.status === 401) {
    removeToken()
    window.location.href = '/login'
    return { success: false, error: '未授权，请重新登录' }
  }

  const result: ApiResponse<T> = await response.json()

  if (!response.ok) {
    return {
      success: false,
      error: result.error || result.message || `请求失败 (${response.status})`,
    }
  }

  return result
}

// API 客户端
export const api = {
  get<T = unknown>(url: string): Promise<ApiResponse<T>> {
    return request<T>('GET', url)
  },

  post<T = unknown>(url: string, data?: unknown): Promise<ApiResponse<T>> {
    return request<T>('POST', url, data)
  },

  put<T = unknown>(url: string, data?: unknown): Promise<ApiResponse<T>> {
    return request<T>('PUT', url, data)
  },

  del<T = unknown>(url: string): Promise<ApiResponse<T>> {
    return request<T>('DELETE', url)
  },
}

// 登录
export async function login(
  username: string,
  password: string
): Promise<ApiResponse<LoginResponse>> {
  const result = await api.post<LoginResponse>('/user/login', { username, password })
  if (result.success && result.data) {
    setToken(result.data.token)
  }
  return result
}

// 退出登录
export function logout(): void {
  removeToken()
  window.location.href = '/login'
}

// 获取当前用户信息
export function getUserInfo(): Promise<ApiResponse<UserInfo>> {
  return api.get<UserInfo>('/user/info')
}

// ── 插件类型 ──────────────────────────────────────────────
export interface PluginParam {
  name: string
  label: string
  param_type: 'int' | 'float'
  default: number
  min?: number
  max?: number
  step?: number
}

export interface Plugin {
  name: string
  description: string
  category: string
  params: PluginParam[]
}

// 获取插件列表
export async function fetchPlugins(): Promise<ApiResponse<Plugin[]>> {
  return api.get<Plugin[]>('/plugins')
}

// 验证 Lua 脚本语法
export async function validateScript(code: string): Promise<ApiResponse<{ valid: boolean; error?: string }>> {
  return api.post('/strategy/validate-script', { code })
}
