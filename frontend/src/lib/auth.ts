import { createSignal } from 'solid-js'
import type { UserInfo } from './api/types'
import { getUserInfo } from './api/auth'
import { getToken, removeToken } from './api/client'

const [user, setUser] = createSignal<UserInfo | null>(null)
const [loading, setLoading] = createSignal(false)

export function isLoggedIn(): boolean {
  return !!getToken()
}

export function getUser(): UserInfo | null {
  return user()
}

export function getUserSignal() {
  return user
}

export function getLoading() {
  return loading
}

export function isAdmin(): boolean {
  return user()?.role === 'admin'
}

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
      removeToken()
      return false
    }
  } catch {
    setUser(null)
    return false
  } finally {
    setLoading(false)
  }
}

export function requireAuth(): boolean {
  if (!isLoggedIn()) {
    window.location.href = '/login'
    return false
  }
  return true
}

export function initAuth(): void {
  if (isLoggedIn()) {
    fetchUser()
  }
}
