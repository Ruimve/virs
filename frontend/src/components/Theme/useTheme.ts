import { useState, useEffect } from 'react'

type Theme = 'light' | 'dark'

const STORAGE_KEY = 'virs-theme'

function getInitialTheme(): Theme {
  const stored = localStorage.getItem(STORAGE_KEY)
  if (stored === 'light' || stored === 'dark') return stored
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

let _theme: Theme = getInitialTheme()
const _listeners = new Set<() => void>()

function applyTheme(t: Theme) {
  document.documentElement.classList.toggle('dark', t === 'dark')
  localStorage.setItem(STORAGE_KEY, t)
}

// Apply on load
applyTheme(_theme)

function notify() {
  _listeners.forEach((l) => l())
}

export function subscribe(listener: () => void) {
  _listeners.add(listener)
  return () => {
    _listeners.delete(listener)
  }
}

export function getTheme(): Theme {
  return _theme
}

export function toggleTheme() {
  _theme = _theme === 'dark' ? 'light' : 'dark'
  applyTheme(_theme)
  notify()
}

export function isDark(): boolean {
  return _theme === 'dark'
}

// React hook
export function useTheme() {
  const [, forceUpdate] = useState(0)
  useEffect(() => {
    return subscribe(() => forceUpdate((v) => v + 1))
  }, [])
  return { theme: getTheme(), isDark: isDark(), toggleTheme }
}
