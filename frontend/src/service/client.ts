import type { ApiResponse } from './types';

const TOKEN_KEY = 'qd_token';

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setToken(token: string): void {
  localStorage.setItem(TOKEN_KEY, token);
}

export function removeToken(): void {
  localStorage.removeItem(TOKEN_KEY);
}

// 查询参数对象，值为 null/undefined 时自动跳过
export type QueryParams = Record<string, string | number | boolean | null | undefined>;

// 统一序列化查询参数，URLSearchParams 保证特殊字符（如 symbol 中的 / 和 :）被正确编码
function buildQuery(params?: QueryParams): string {
  if (!params) return '';
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    // 只序列化原始类型：null/undefined 跳过，误传对象（如 RequestInit）也不会污染 query
    if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
      search.append(key, String(value));
    }
  }
  const str = search.toString();
  return str ? `?${str}` : '';
}

async function request<T>(
  method: string,
  url: string,
  data?: unknown,
  init?: RequestInit,
): Promise<ApiResponse<T>> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };

  const token = getToken();
  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const config: RequestInit = {
    ...init,
    method,
    headers: { ...headers, ...(init?.headers as Record<string, string> | undefined) },
  };

  if (data !== undefined && method !== 'GET') {
    config.body = JSON.stringify(data);
  }

  const response = await fetch(`/api${url}`, config);

  if (response.status === 401) {
    removeToken();
    window.dispatchEvent(new CustomEvent('auth:unauthorized'));
    return { success: false, message: '未授权，请重新登录' };
  }

  const result: ApiResponse<T> = await response.json();

  if (!response.ok) {
    return {
      success: false,
      message: result.message || `请求失败 (${response.status})`,
    };
  }

  return result;
}

export const api = {
  get<T = unknown>(url: string, params?: QueryParams, init?: RequestInit): Promise<ApiResponse<T>> {
    return request<T>('GET', url + buildQuery(params), undefined, init);
  },

  post<T = unknown>(
    url: string,
    params?: QueryParams,
    data?: unknown,
    init?: RequestInit,
  ): Promise<ApiResponse<T>> {
    return request<T>('POST', url + buildQuery(params), data, init);
  },

  put<T = unknown>(
    url: string,
    params?: QueryParams,
    data?: unknown,
    init?: RequestInit,
  ): Promise<ApiResponse<T>> {
    return request<T>('PUT', url + buildQuery(params), data, init);
  },

  del<T = unknown>(url: string, params?: QueryParams, init?: RequestInit): Promise<ApiResponse<T>> {
    return request<T>('DELETE', url + buildQuery(params), undefined, init);
  },
};
