import { useState, useEffect } from 'react'
import type { UserInfo } from '../service/types'
import { getUserInfo } from '../service/auth'
import { getToken, removeToken } from '../service/client'

// Module-level state with subscriber pattern for React
let _user: UserInfo | null = null
let _loading = false
const _listeners = new Set<() => void>()

function notify() {
  _listeners.forEach((l) => l())
}

export function subscribe(listener: () => void) {
  _listeners.add(listener)
  return () => {
    _listeners.delete(listener)
  }
}

export function isLoggedIn(): boolean {
  return !!getToken()
}

export function getUser(): UserInfo | null {
  return _user
}

export function getLoading(): boolean {
  return _loading
}

export function isAdmin(): boolean {
  return _user?.role === 'admin'
}

export async function fetchUser(): Promise<boolean> {
  if (!isLoggedIn()) {
    _user = null
    notify()
    return false
  }

  _loading = true
  notify()
  try {
    const result = await getUserInfo()
    if (result.success && result.data) {
      _user = result.data
      notify()
      return true
    } else {
      _user = null
      removeToken()
      notify()
      return false
    }
  } catch {
    _user = null
    notify()
    return false
  } finally {
    _loading = false
    notify()
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

// React hook to subscribe to auth state changes
export function useAuthState() {
  const [, forceUpdate] = useState(0)
  useEffect(() => {
    return subscribe(() => forceUpdate((v) => v + 1))
  }, [])
  return { user: getUser(), loading: getLoading() }
}
