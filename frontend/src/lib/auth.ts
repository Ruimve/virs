import { createSignal } from 'solid-js'
import type { UserInfo } from './api'
import { getUserInfo } from './api'

const TOKEN_KEY = 'qd_token'

const [user, setUser] = createSignal<UserInfo | null>(null)
const [loading, setLoading] = createSignal(false)

// 检查 token 是否存在
export function isLoggedIn(): boolean {
  return !!localStorage.getItem(TOKEN_KEY)
}

// 获取当前用户信息
export function getUser(): UserInfo | null {
  return user()
}

// 获取用户信号
export function getUserSignal() {
  return user
}

// 获取加载状态
export function getLoading() {
  return loading
}

// 检查是否管理员
export function isAdmin(): boolean {
  return user()?.role === 'admin'
}

// 获取用户信息 (异步)
export async function fetchUser(): Promise<boolean> {
  if (!isLoggedIn()) {
    setUser(null)
    return false
  }

  setLoading(true)
  try {
    const result = await getUserInfo()
    if (result.success && result.data) {
      setUser(result.data)
      return true
    } else {
      setUser(null)
      localStorage.removeItem(TOKEN_KEY)
      return false
    }
  } catch {
    setUser(null)
    return false
  } finally {
    setLoading(false)
  }
}

// 路由守卫 - 未登录跳转 /login
export function requireAuth(): boolean {
  if (!isLoggedIn()) {
    window.location.href = '/login'
    return false
  }
  return true
}

// 初始化认证状态
export function initAuth(): void {
  if (isLoggedIn()) {
    fetchUser()
  }
}
