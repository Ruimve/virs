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
      message: result.message || result.message || `请求失败 (${response.status})`,
    };
  }

  return result;
}

export const api = {
  get<T = unknown>(url: string, init?: RequestInit): Promise<ApiResponse<T>> {
    return request<T>('GET', url, undefined, init);
  },

  post<T = unknown>(url: string, data?: unknown, init?: RequestInit): Promise<ApiResponse<T>> {
    return request<T>('POST', url, data, init);
  },

  put<T = unknown>(url: string, data?: unknown, init?: RequestInit): Promise<ApiResponse<T>> {
    return request<T>('PUT', url, data, init);
  },

  del<T = unknown>(url: string, init?: RequestInit): Promise<ApiResponse<T>> {
    return request<T>('DELETE', url, undefined, init);
  },
};
